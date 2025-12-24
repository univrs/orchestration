//! OCI Runtime Specification types.
//!
//! This module defines the OCI runtime specification structures for generating
//! config.json files. Based on OCI Runtime Spec v1.0.2.
//!
//! Reference: https://github.com/opencontainers/runtime-spec/blob/main/config.md

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OCI Runtime Specification - the root config.json structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OciSpec {
    /// OCI spec version (e.g., "1.0.2")
    pub oci_version: String,

    /// Root filesystem configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Root>,

    /// Container process configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<Process>,

    /// Container hostname
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,

    /// Mount points
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mounts: Vec<Mount>,

    /// Linux-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linux: Option<Linux>,

    /// Annotations (key-value metadata)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

impl Default for OciSpec {
    fn default() -> Self {
        Self {
            oci_version: "1.0.2".to_string(),
            root: None,
            process: None,
            hostname: None,
            mounts: Vec::new(),
            linux: None,
            annotations: HashMap::new(),
        }
    }
}

/// Root filesystem configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Root {
    /// Path to the root filesystem (relative to bundle)
    pub path: String,

    /// Whether the root filesystem is read-only
    #[serde(default)]
    pub readonly: bool,
}

impl Default for Root {
    fn default() -> Self {
        Self {
            path: "rootfs".to_string(),
            readonly: false,
        }
    }
}

/// Process configuration for the container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Process {
    /// Whether to allocate a terminal
    #[serde(default)]
    pub terminal: bool,

    /// User and group IDs
    pub user: User,

    /// Command line arguments
    pub args: Vec<String>,

    /// Environment variables
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,

    /// Working directory
    pub cwd: String,

    /// Process capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Capabilities>,

    /// Resource limits (rlimits)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rlimits: Vec<Rlimit>,

    /// No new privileges flag
    #[serde(default)]
    pub no_new_privileges: bool,
}

impl Default for Process {
    fn default() -> Self {
        Self {
            terminal: false,
            user: User::default(),
            args: vec!["/bin/sh".to_string()],
            env: vec![
                "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".to_string(),
                "TERM=xterm".to_string(),
            ],
            cwd: "/".to_string(),
            capabilities: Some(Capabilities::default()),
            rlimits: Vec::new(),
            no_new_privileges: true,
        }
    }
}

/// User identity for the process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    /// User ID
    pub uid: u32,

    /// Group ID
    pub gid: u32,

    /// Umask (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub umask: Option<u32>,

    /// Additional group IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_gids: Vec<u32>,
}

impl Default for User {
    fn default() -> Self {
        Self {
            uid: 0,
            gid: 0,
            umask: Some(0o022),
            additional_gids: Vec::new(),
        }
    }
}

/// Linux capabilities for the process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bounding: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effective: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inheritable: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permitted: Vec<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ambient: Vec<String>,
}

impl Default for Capabilities {
    fn default() -> Self {
        let caps = vec![
            "CAP_AUDIT_WRITE".to_string(),
            "CAP_KILL".to_string(),
            "CAP_NET_BIND_SERVICE".to_string(),
        ];
        Self {
            bounding: caps.clone(),
            effective: caps.clone(),
            inheritable: caps.clone(),
            permitted: caps.clone(),
            ambient: caps,
        }
    }
}

impl Capabilities {
    /// Create capabilities with all capabilities granted (privileged container).
    pub fn privileged() -> Self {
        let all_caps: Vec<String> = vec![
            "CAP_AUDIT_CONTROL",
            "CAP_AUDIT_READ",
            "CAP_AUDIT_WRITE",
            "CAP_BLOCK_SUSPEND",
            "CAP_CHOWN",
            "CAP_DAC_OVERRIDE",
            "CAP_DAC_READ_SEARCH",
            "CAP_FOWNER",
            "CAP_FSETID",
            "CAP_IPC_LOCK",
            "CAP_IPC_OWNER",
            "CAP_KILL",
            "CAP_LEASE",
            "CAP_LINUX_IMMUTABLE",
            "CAP_MAC_ADMIN",
            "CAP_MAC_OVERRIDE",
            "CAP_MKNOD",
            "CAP_NET_ADMIN",
            "CAP_NET_BIND_SERVICE",
            "CAP_NET_BROADCAST",
            "CAP_NET_RAW",
            "CAP_SETFCAP",
            "CAP_SETGID",
            "CAP_SETPCAP",
            "CAP_SETUID",
            "CAP_SYSLOG",
            "CAP_SYS_ADMIN",
            "CAP_SYS_BOOT",
            "CAP_SYS_CHROOT",
            "CAP_SYS_MODULE",
            "CAP_SYS_NICE",
            "CAP_SYS_PACCT",
            "CAP_SYS_PTRACE",
            "CAP_SYS_RAWIO",
            "CAP_SYS_RESOURCE",
            "CAP_SYS_TIME",
            "CAP_SYS_TTY_CONFIG",
            "CAP_WAKE_ALARM",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        Self {
            bounding: all_caps.clone(),
            effective: all_caps.clone(),
            inheritable: all_caps.clone(),
            permitted: all_caps.clone(),
            ambient: all_caps,
        }
    }
}

/// Resource limit (rlimit).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rlimit {
    #[serde(rename = "type")]
    pub limit_type: String,
    pub hard: u64,
    pub soft: u64,
}

/// Mount point configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    /// Destination path in the container
    pub destination: String,

    /// Mount type (e.g., "bind", "proc", "tmpfs")
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub mount_type: Option<String>,

    /// Source path (for bind mounts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,

    /// Mount options
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

impl Mount {
    /// Create a proc mount.
    pub fn proc() -> Self {
        Self {
            destination: "/proc".to_string(),
            mount_type: Some("proc".to_string()),
            source: Some("proc".to_string()),
            options: vec![],
        }
    }

    /// Create a tmpfs mount.
    pub fn tmpfs(destination: &str) -> Self {
        Self {
            destination: destination.to_string(),
            mount_type: Some("tmpfs".to_string()),
            source: Some("tmpfs".to_string()),
            options: vec![
                "nosuid".to_string(),
                "strictatime".to_string(),
                "mode=755".to_string(),
                "size=65536k".to_string(),
            ],
        }
    }

    /// Create a devpts mount.
    pub fn devpts() -> Self {
        Self {
            destination: "/dev/pts".to_string(),
            mount_type: Some("devpts".to_string()),
            source: Some("devpts".to_string()),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "newinstance".to_string(),
                "ptmxmode=0666".to_string(),
                "mode=0620".to_string(),
            ],
        }
    }

    /// Create a sysfs mount.
    pub fn sysfs() -> Self {
        Self {
            destination: "/sys".to_string(),
            mount_type: Some("sysfs".to_string()),
            source: Some("sysfs".to_string()),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                "ro".to_string(),
            ],
        }
    }

    /// Create a cgroup mount.
    pub fn cgroup() -> Self {
        Self {
            destination: "/sys/fs/cgroup".to_string(),
            mount_type: Some("cgroup".to_string()),
            source: Some("cgroup".to_string()),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                "relatime".to_string(),
                "ro".to_string(),
            ],
        }
    }

    /// Create a bind mount.
    pub fn bind(source: &str, destination: &str, readonly: bool) -> Self {
        let mut options = vec!["bind".to_string()];
        if readonly {
            options.push("ro".to_string());
        }
        Self {
            destination: destination.to_string(),
            mount_type: Some("bind".to_string()),
            source: Some(source.to_string()),
            options,
        }
    }

    /// Create a mqueue mount.
    pub fn mqueue() -> Self {
        Self {
            destination: "/dev/mqueue".to_string(),
            mount_type: Some("mqueue".to_string()),
            source: Some("mqueue".to_string()),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
            ],
        }
    }

    /// Create a shm mount.
    pub fn shm() -> Self {
        Self {
            destination: "/dev/shm".to_string(),
            mount_type: Some("tmpfs".to_string()),
            source: Some("shm".to_string()),
            options: vec![
                "nosuid".to_string(),
                "noexec".to_string(),
                "nodev".to_string(),
                "mode=1777".to_string(),
                "size=65536k".to_string(),
            ],
        }
    }
}

/// Linux-specific configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Linux {
    /// Linux namespaces
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespaces: Vec<Namespace>,

    /// User ID mappings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uid_mappings: Vec<IdMapping>,

    /// Group ID mappings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gid_mappings: Vec<IdMapping>,

    /// Device configuration
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,

    /// Cgroup path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cgroups_path: Option<String>,

    /// Resource limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,

    /// Masked paths (hidden from container)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masked_paths: Vec<String>,

    /// Read-only paths
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readonly_paths: Vec<String>,

    /// Seccomp configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seccomp: Option<Seccomp>,
}

/// Linux namespace configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    /// Namespace type
    #[serde(rename = "type")]
    pub ns_type: String,

    /// Path to an existing namespace (for joining)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Namespace {
    pub fn pid() -> Self {
        Self { ns_type: "pid".to_string(), path: None }
    }

    pub fn network() -> Self {
        Self { ns_type: "network".to_string(), path: None }
    }

    pub fn ipc() -> Self {
        Self { ns_type: "ipc".to_string(), path: None }
    }

    pub fn uts() -> Self {
        Self { ns_type: "uts".to_string(), path: None }
    }

    pub fn mount() -> Self {
        Self { ns_type: "mount".to_string(), path: None }
    }

    pub fn user() -> Self {
        Self { ns_type: "user".to_string(), path: None }
    }

    pub fn cgroup() -> Self {
        Self { ns_type: "cgroup".to_string(), path: None }
    }
}

/// User/Group ID mapping for user namespaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdMapping {
    pub container_id: u32,
    pub host_id: u32,
    pub size: u32,
}

/// Linux device configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// Device type (c for char, b for block)
    #[serde(rename = "type")]
    pub device_type: String,

    /// Path in the container
    pub path: String,

    /// Major device number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major: Option<i64>,

    /// Minor device number
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor: Option<i64>,

    /// File mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_mode: Option<u32>,

    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,

    /// Group ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gid: Option<u32>,
}

impl Device {
    /// Create /dev/null device.
    pub fn null() -> Self {
        Self {
            device_type: "c".to_string(),
            path: "/dev/null".to_string(),
            major: Some(1),
            minor: Some(3),
            file_mode: Some(0o666),
            uid: None,
            gid: None,
        }
    }

    /// Create /dev/zero device.
    pub fn zero() -> Self {
        Self {
            device_type: "c".to_string(),
            path: "/dev/zero".to_string(),
            major: Some(1),
            minor: Some(5),
            file_mode: Some(0o666),
            uid: None,
            gid: None,
        }
    }

    /// Create /dev/full device.
    pub fn full() -> Self {
        Self {
            device_type: "c".to_string(),
            path: "/dev/full".to_string(),
            major: Some(1),
            minor: Some(7),
            file_mode: Some(0o666),
            uid: None,
            gid: None,
        }
    }

    /// Create /dev/random device.
    pub fn random() -> Self {
        Self {
            device_type: "c".to_string(),
            path: "/dev/random".to_string(),
            major: Some(1),
            minor: Some(8),
            file_mode: Some(0o666),
            uid: None,
            gid: None,
        }
    }

    /// Create /dev/urandom device.
    pub fn urandom() -> Self {
        Self {
            device_type: "c".to_string(),
            path: "/dev/urandom".to_string(),
            major: Some(1),
            minor: Some(9),
            file_mode: Some(0o666),
            uid: None,
            gid: None,
        }
    }

    /// Create /dev/tty device.
    pub fn tty() -> Self {
        Self {
            device_type: "c".to_string(),
            path: "/dev/tty".to_string(),
            major: Some(5),
            minor: Some(0),
            file_mode: Some(0o666),
            uid: None,
            gid: None,
        }
    }
}

/// Resource limits for cgroups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Resources {
    /// CPU resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<CpuResources>,

    /// Memory resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryResources>,

    /// Block I/O resources
    #[serde(rename = "blockIO", skip_serializing_if = "Option::is_none")]
    pub block_io: Option<BlockIoResources>,

    /// PIDs limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pids: Option<PidsResources>,
}

/// CPU resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuResources {
    /// CPU shares (relative weight)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<u64>,

    /// CPU quota in microseconds per period
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota: Option<i64>,

    /// CPU period in microseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,

    /// Realtime runtime in microseconds
    #[serde(rename = "realtimeRuntime", skip_serializing_if = "Option::is_none")]
    pub realtime_runtime: Option<i64>,

    /// Realtime period in microseconds
    #[serde(rename = "realtimePeriod", skip_serializing_if = "Option::is_none")]
    pub realtime_period: Option<u64>,

    /// CPUs to use (e.g., "0-2,7")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,

    /// Memory nodes to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mems: Option<String>,
}

impl CpuResources {
    /// Create CPU resources from cores (e.g., 0.5 = half a core, 2.0 = 2 cores).
    pub fn from_cores(cores: f32) -> Self {
        let period: u64 = 100_000; // 100ms period
        let quota = (cores * period as f32) as i64;

        Self {
            shares: Some((cores * 1024.0) as u64),
            quota: Some(quota),
            period: Some(period),
            ..Default::default()
        }
    }
}

/// Memory resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryResources {
    /// Memory limit in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,

    /// Memory reservation (soft limit) in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservation: Option<i64>,

    /// Total memory limit (memory + swap)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swap: Option<i64>,

    /// Kernel memory limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel: Option<i64>,

    /// Kernel TCP memory limit
    #[serde(rename = "kernelTCP", skip_serializing_if = "Option::is_none")]
    pub kernel_tcp: Option<i64>,

    /// Swappiness
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swappiness: Option<u64>,

    /// Disable OOM killer
    #[serde(rename = "disableOOMKiller", skip_serializing_if = "Option::is_none")]
    pub disable_oom_killer: Option<bool>,
}

impl MemoryResources {
    /// Create memory resources from MB.
    pub fn from_mb(mb: u64) -> Self {
        let bytes = mb as i64 * 1024 * 1024;
        Self {
            limit: Some(bytes),
            reservation: Some(bytes / 2), // Soft limit at 50%
            swap: Some(bytes), // No swap (total = memory limit)
            ..Default::default()
        }
    }
}

/// Block I/O resource limits.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockIoResources {
    /// I/O weight
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,

    /// Leaf weight
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leaf_weight: Option<u16>,

    /// Throttle read BPS device limits
    #[serde(rename = "throttleReadBpsDevice", default, skip_serializing_if = "Vec::is_empty")]
    pub throttle_read_bps_device: Vec<ThrottleDevice>,

    /// Throttle write BPS device limits
    #[serde(rename = "throttleWriteBpsDevice", default, skip_serializing_if = "Vec::is_empty")]
    pub throttle_write_bps_device: Vec<ThrottleDevice>,
}

/// Throttle device configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleDevice {
    pub major: i64,
    pub minor: i64,
    pub rate: u64,
}

/// PIDs resource limit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PidsResources {
    /// Maximum number of PIDs
    pub limit: i64,
}

/// Seccomp configuration (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Seccomp {
    /// Default action
    pub default_action: String,

    /// Architectures
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<String>,

    /// Syscall rules
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub syscalls: Vec<SeccompSyscall>,
}

/// Seccomp syscall rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompSyscall {
    /// Syscall names
    pub names: Vec<String>,

    /// Action for these syscalls
    pub action: String,
}

impl Default for Seccomp {
    fn default() -> Self {
        Self {
            default_action: "SCMP_ACT_ERRNO".to_string(),
            architectures: vec![
                "SCMP_ARCH_X86_64".to_string(),
                "SCMP_ARCH_X86".to_string(),
                "SCMP_ARCH_AARCH64".to_string(),
            ],
            syscalls: vec![SeccompSyscall {
                names: vec![
                    "accept", "accept4", "access", "adjtimex", "alarm", "bind", "brk",
                    "capget", "capset", "chdir", "chmod", "chown", "chroot", "clock_getres",
                    "clock_gettime", "clock_nanosleep", "close", "connect", "copy_file_range",
                    "creat", "dup", "dup2", "dup3", "epoll_create", "epoll_create1",
                    "epoll_ctl", "epoll_pwait", "epoll_wait", "eventfd", "eventfd2",
                    "execve", "execveat", "exit", "exit_group", "faccessat", "fadvise64",
                    "fallocate", "fanotify_mark", "fchdir", "fchmod", "fchmodat", "fchown",
                    "fchownat", "fcntl", "fdatasync", "fgetxattr", "flistxattr", "flock",
                    "fork", "fremovexattr", "fsetxattr", "fstat", "fstatfs", "fsync",
                    "ftruncate", "futex", "futimesat", "getcpu", "getcwd", "getdents",
                    "getdents64", "getegid", "geteuid", "getgid", "getgroups", "getitimer",
                    "getpeername", "getpgid", "getpgrp", "getpid", "getppid", "getpriority",
                    "getrandom", "getresgid", "getresuid", "getrlimit", "get_robust_list",
                    "getrusage", "getsid", "getsockname", "getsockopt", "get_thread_area",
                    "gettid", "gettimeofday", "getuid", "getxattr", "inotify_add_watch",
                    "inotify_init", "inotify_init1", "inotify_rm_watch", "io_cancel",
                    "ioctl", "io_destroy", "io_getevents", "ioprio_get", "ioprio_set",
                    "io_setup", "io_submit", "kill", "lchown", "lgetxattr", "link",
                    "linkat", "listen", "listxattr", "llistxattr", "lremovexattr", "lseek",
                    "lsetxattr", "lstat", "madvise", "memfd_create", "mincore", "mkdir",
                    "mkdirat", "mknod", "mknodat", "mlock", "mlock2", "mlockall", "mmap",
                    "mprotect", "mq_getsetattr", "mq_notify", "mq_open", "mq_timedreceive",
                    "mq_timedsend", "mq_unlink", "mremap", "msgctl", "msgget", "msgrcv",
                    "msgsnd", "msync", "munlock", "munlockall", "munmap", "nanosleep",
                    "newfstatat", "open", "openat", "pause", "pipe", "pipe2", "poll",
                    "ppoll", "prctl", "pread64", "preadv", "prlimit64", "pselect6",
                    "pwrite64", "pwritev", "read", "readahead", "readlink", "readlinkat",
                    "readv", "recv", "recvfrom", "recvmmsg", "recvmsg", "remap_file_pages",
                    "removexattr", "rename", "renameat", "renameat2", "restart_syscall",
                    "rmdir", "rt_sigaction", "rt_sigpending", "rt_sigprocmask", "rt_sigqueueinfo",
                    "rt_sigreturn", "rt_sigsuspend", "rt_sigtimedwait", "rt_tgsigqueueinfo",
                    "sched_getaffinity", "sched_getattr", "sched_getparam", "sched_get_priority_max",
                    "sched_get_priority_min", "sched_getscheduler", "sched_rr_get_interval",
                    "sched_setaffinity", "sched_setattr", "sched_setparam", "sched_setscheduler",
                    "sched_yield", "seccomp", "select", "semctl", "semget", "semop",
                    "semtimedop", "send", "sendfile", "sendmmsg", "sendmsg", "sendto",
                    "setfsgid", "setfsuid", "setgid", "setgroups", "setitimer", "setpgid",
                    "setpriority", "setregid", "setresgid", "setresuid", "setreuid",
                    "setrlimit", "set_robust_list", "setsid", "setsockopt", "set_thread_area",
                    "set_tid_address", "setuid", "setxattr", "shmat", "shmctl", "shmdt",
                    "shmget", "shutdown", "sigaltstack", "signalfd", "signalfd4", "sigreturn",
                    "socket", "socketpair", "splice", "stat", "statfs", "statx", "symlink",
                    "symlinkat", "sync", "sync_file_range", "syncfs", "sysinfo", "tee",
                    "tgkill", "time", "timer_create", "timer_delete", "timerfd_create",
                    "timerfd_gettime", "timerfd_settime", "timer_getoverrun", "timer_gettime",
                    "timer_settime", "times", "tkill", "truncate", "umask", "uname",
                    "unlink", "unlinkat", "utime", "utimensat", "utimes", "vfork", "vmsplice",
                    "wait4", "waitid", "waitpid", "write", "writev",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
                action: "SCMP_ACT_ALLOW".to_string(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_spec() {
        let spec = OciSpec::default();
        assert_eq!(spec.oci_version, "1.0.2");
    }

    #[test]
    fn test_spec_serialization() {
        let spec = OciSpec {
            oci_version: "1.0.2".to_string(),
            root: Some(Root::default()),
            process: Some(Process::default()),
            hostname: Some("test-container".to_string()),
            mounts: vec![Mount::proc(), Mount::tmpfs("/dev")],
            linux: Some(Linux {
                namespaces: vec![
                    Namespace::pid(),
                    Namespace::network(),
                    Namespace::mount(),
                ],
                ..Default::default()
            }),
            annotations: HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&spec).unwrap();
        assert!(json.contains("ociVersion"));
        assert!(json.contains("1.0.2"));
        assert!(json.contains("rootfs"));
    }

    #[test]
    fn test_cpu_resources_from_cores() {
        let cpu = CpuResources::from_cores(0.5);
        assert_eq!(cpu.period, Some(100_000));
        assert_eq!(cpu.quota, Some(50_000)); // 0.5 * 100_000
    }

    #[test]
    fn test_memory_resources_from_mb() {
        let mem = MemoryResources::from_mb(256);
        assert_eq!(mem.limit, Some(256 * 1024 * 1024));
    }
}
