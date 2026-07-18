#define _GNU_SOURCE
#include <stdio.h>
#include <stdarg.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <string.h>
#include <unistd.h>

// using the va_list version to pass variadic arguments to the original function
static void (*real_av_vlog) (void*, int, const char*, va_list) = NULL;

bool is_frame_msg(const char* msg) {
    const char FRAME_PREF[] = "frame";
    return strncmp(msg, FRAME_PREF, strlen(FRAME_PREF)) == 0;
}

static void init_intercept(void) {
    if (real_av_vlog) return;
    real_av_vlog = dlsym(RTLD_NEXT, "av_vlog");
    if (!real_av_vlog) {
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
    } else {
        va_start(args, fmt);
        real_av_vlog(avcl, level, fmt, args);
        va_end(args);
    }
}