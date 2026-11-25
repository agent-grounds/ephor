#define _GNU_SOURCE
/* §REQ-003-test-isolation.5: retain the actual fixture writer; never fabricate an exec failure. */
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

/* Each selected test constructs one runner. Reap our own holder at process
 * exit, so even a quickly failing test leaves no running child or zombie.
 */
static pid_t holder;
__attribute__((destructor)) static void reap_holder(void) {
    if (holder > 0) {
        while (syscall(SYS_wait4, holder, NULL, 0, NULL) < 0 && errno == EINTR) {}
    }
}

/* Test instrumentation only: preserve the real fs::write descriptor in a
 * fork child for 60ms, immediately before the test closes its own copy.
 * O_CLOEXEC does not close a descriptor at fork. No file is rewritten.
 * Post-fork child code uses syscalls only, then exits without libc cleanup.
 */
static void record(const char *event, const char *path, int value) {
    const char *trace = getenv("EPHOR_271_TRACE");
    if (!trace) return;
    char message[PATH_MAX + 128];
    int length = snprintf(message, sizeof(message), "%s value=%d path=%s\n",
                          event, value, path);
    if (length <= 0 || (size_t)length >= sizeof(message)) return;
    int fd = syscall(SYS_openat, AT_FDCWD, trace,
                     O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (fd < 0) return;
    syscall(SYS_write, fd, message, (size_t)length);
    syscall(SYS_close, fd);
}

int close(int fd) {
    int saved_errno = errno;
    if (getenv("EPHOR_271_TRACE")) {
        char link[64], path[PATH_MAX];
        snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
        ssize_t length = readlink(link, path, sizeof(path) - 1);
        int flags = fcntl(fd, F_GETFL);
        if (length > 0 && flags >= 0 && (flags & O_ACCMODE) != O_RDONLY) {
            path[length] = '\0';
            const char *base = strrchr(path, '/');
            if (base && strcmp(base + 1, "runtime-stub") == 0) {
                int descriptor_flags = fcntl(fd, F_GETFD);
                pid_t child = syscall(SYS_fork);
                if (child == 0) {
                    struct timespec remaining = { .tv_sec = 0, .tv_nsec = 60000000 };
                    while (syscall(SYS_nanosleep, &remaining, &remaining) < 0 && errno == EINTR) {}
                    syscall(SYS_close, fd);
                    syscall(SYS_exit, 0);
                    __builtin_unreachable();
                }
                if (child > 0) {
                    holder = child;
                    record("fork-held-writable-fd; FD_CLOEXEC", path,
                           (descriptor_flags & FD_CLOEXEC) != 0);
                } else {
                    record("INJECTION-ERROR fork errno", path, errno);
                }
            }
        }
    }
    errno = saved_errno;
    return syscall(SYS_close, fd);
}

/* The shell's real exec is allowed to fail normally. Record the kernel's
 * errno separately, independently of shell wording and error formatting. */
int execve(const char *path, char *const argv[], char *const envp[]) {
    int result = syscall(SYS_execve, path, argv, envp);
    int failure = errno;
    if (failure == ETXTBSY) record("execve ETXTBSY errno", path, failure);
    errno = failure;
    return result;
}
