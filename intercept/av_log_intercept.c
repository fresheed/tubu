#define _GNU_SOURCE
#include <stdio.h>
#include <stdarg.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <string.h>
#include <unistd.h>

// using the va_list version to pass variadic arguments to the original function
static void (*real_av_vlog) (void*, int, const char*, va_list) = NULL;
_Atomic int* frames_amount = NULL;

bool is_frame_msg(const char* msg) {
    const char FRAME_PREF[] = "frame=";
    return strncmp(msg, FRAME_PREF, strlen(FRAME_PREF)) == 0;
}

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <fcntl.h>      // O_CREAT, O_RDWR
#include <sys/mman.h>   // shm_open, mmap
#include <sys/stat.h>   // mode constants
#include <unistd.h>     // ftruncate, close, _exit

#define SHM_NAME "/tubu_shared"

_Atomic int* setup_shared_memory() {
    int fd = shm_open(SHM_NAME, O_RDWR, 0600);
    if (fd == -1) {
        perror("shm_open");
        return NULL;
    }

    _Atomic int* ptr = mmap(
        NULL, // we don't care about the logical address for this process
        sizeof(_Atomic int),
        PROT_READ | PROT_WRITE,
        MAP_SHARED, 
        fd, 
        0 // but C and Rust must agree on where the data is within the shared memory
    );

    close(fd);  // can be closed immediately according to `man mmap`

    if (ptr == MAP_FAILED) {
        perror("mmap");
        return NULL;
    }

    return ptr;
}

static void init_intercept(void) {
    if (real_av_vlog) return;
    real_av_vlog = dlsym(RTLD_NEXT, "av_vlog");
    frames_amount = setup_shared_memory();
    if (!real_av_vlog || !frames_amount) {
        fprintf(stderr, "Cannot setup the interceptor!\n");
        _exit(1);
    }    
}

// We can only intercept av_log: the underlying av_vlog does not go through PLT
void av_log(void *avcl, int level, const char *fmt, ...) {
    if (!real_av_vlog) init_intercept();

    va_list args;
    va_start(args, fmt);
    char msg[1024];
    vsnprintf(msg, sizeof(msg), fmt, args);
    va_end(args);

    if (is_frame_msg(msg)) {
        printf("INTERCEPTED\n");
        atomic_fetch_add_explicit(frames_amount, 1, memory_order_relaxed);
    } else {
        va_start(args, fmt);
        real_av_vlog(avcl, level, fmt, args);
        va_end(args);
    }
}