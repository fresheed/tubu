#define _GNU_SOURCE
#include <stdio.h>
#include <stdarg.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <string.h>
#include <unistd.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <inttypes.h>

#define SHM_NAME "/tubu_shared"

// using the va_list version to pass variadic arguments to the original function
static void (*real_av_vlog) (void*, int, const char*, va_list) = NULL;
_Atomic u_int32_t* frames_amount = NULL;

_Atomic u_int32_t* setup_shared_memory() {
    int shm = shm_open(SHM_NAME, O_RDWR, 0600);
    if (shm == -1) {
        perror("shm_open");
        return NULL;
    }

    _Atomic u_int32_t* ptr = mmap(
        NULL, // we don't care about the logical address for this process
        sizeof(_Atomic u_int32_t),
        PROT_READ | PROT_WRITE,
        MAP_SHARED, 
        shm, 
        0 // but C and Rust must agree on where the data is within the shared memory
    );

    close(shm);  // can be closed immediately according to `man mmap`

    if (ptr == MAP_FAILED) {
        perror("mmap");
        return NULL;
    }

    // No need for teardown upon ffmpeg termination:
    // 1) munmap is not required - see man mmap:
    // "region is also automatically unmapped when the process is terminated"
    // 2) shm_unlink is done by tubu who created shm object
    // 3) local fd is already closed

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

    u_int32_t f;
    if (sscanf(msg, "frame=%" SCNu32, &f) == 1) {
        atomic_store_explicit(frames_amount, f, memory_order_relaxed);
    } else {
        va_start(args, fmt);
        real_av_vlog(avcl, level, fmt, args);
        va_end(args);
    }
}