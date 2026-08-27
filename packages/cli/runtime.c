#include <stdint.h>

static int g_argc = 0;
static char** g_argv = 0;

int32_t mantis_get_argc(void) {
    return g_argc;
}

char** mantis_get_argv(void) {
    return g_argv;
}

__attribute__((constructor))
static void init_args(int argc, char** argv, char** envp) {
    (void)envp;
    g_argc = argc;
    g_argv = argv;
}
