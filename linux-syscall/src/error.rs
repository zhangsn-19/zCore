use core::fmt;
use rcore_fs::vfs::FsError;
use zircon_object::ZxError;

pub type LxResult<T> = Result<T, SysError>;
pub type SysResult = LxResult<usize>;

#[allow(dead_code)]
#[repr(isize)]
#[derive(Debug)]
pub enum SysError {
    EUNDEF = 0,
    EPERM = 1,
    ENOENT = 2,
    ESRCH = 3,
    EINTR = 4,
    EIO = 5,
    ENXIO = 6,
    E2BIG = 7,
    ENOEXEC = 8,
    EBADF = 9,
    ECHILD = 10,
    EAGAIN = 11,
    ENOMEM = 12,
    EACCES = 13,
    EFAULT = 14,
    ENOTBLK = 15,
    EBUSY = 16,
    EEXIST = 17,
    EXDEV = 18,
    ENODEV = 19,
    ENOTDIR = 20,
    EISDIR = 21,
    EINVAL = 22,
    ENFILE = 23,
    EMFILE = 24,
    ENOTTY = 25,
    ETXTBSY = 26,
    EFBIG = 27,
    ENOSPC = 28,
    ESPIPE = 29,
    EROFS = 30,
    EMLINK = 31,
    EPIPE = 32,
    EDOM = 33,
    ERANGE = 34,
    EDEADLK = 35,
    ENAMETOOLONG = 36,
    ENOLCK = 37,
    ENOSYS = 38,
    ENOTEMPTY = 39,
    ELOOP = 40,
    ENOTSOCK = 80,
    ENOPROTOOPT = 92,
    EPFNOSUPPORT = 96,
    EAFNOSUPPORT = 97,
    ENOBUFS = 105,
    EISCONN = 106,
    ENOTCONN = 107,
    ECONNREFUSED = 111,
}

#[allow(non_snake_case)]
impl fmt::Display for SysError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::SysError::*;
        let explain = match self {
            EPERM => "Operation not permitted",
            ENOENT => "No such file or directory",
            ESRCH => "No such process",
            EINTR => "Interrupted system call",
            EIO => "I/O error",
            ENXIO => "No such device or address",
            E2BIG => "Argument list too long",
            ENOEXEC => "Exec format error",
            EBADF => "Bad file number",
            ECHILD => "No child processes",
            EAGAIN => "Try again",
            ENOMEM => "Out of memory",
            EACCES => "Permission denied",
            EFAULT => "Bad address",
            ENOTBLK => "Block device required",
            EBUSY => "Device or resource busy",
            EEXIST => "File exists",
            EXDEV => "Cross-device link",
            ENODEV => "No such device",
            ENOTDIR => "Not a directory",
            EISDIR => "Is a directory",
            EINVAL => "Invalid argument",
            ENFILE => "File table overflow",
            EMFILE => "Too many open files",
            ENOTTY => "Not a typewriter",
            ETXTBSY => "Text file busy",
            EFBIG => "File too large",
            ENOSPC => "No space left on device",
            ESPIPE => "Illegal seek",
            EROFS => "Read-only file system",
            EMLINK => "Too many links",
            EPIPE => "Broken pipe",
            EDOM => "Math argument out of domain of func",
            ERANGE => "Math result not representable",
            EDEADLK => "Resource deadlock would occur",
            ENAMETOOLONG => "File name too long",
            ENOLCK => "No record locks available",
            ENOSYS => "Function not implemented",
            ENOTEMPTY => "Directory not empty",
            ELOOP => "Too many symbolic links encountered",
            ENOTSOCK => "Socket operation on non-socket",
            ENOPROTOOPT => "Protocol not available",
            EPFNOSUPPORT => "Protocol family not supported",
            EAFNOSUPPORT => "Address family not supported by protocol",
            ENOBUFS => "No buffer space available",
            EISCONN => "Transport endpoint is already connected",
            ENOTCONN => "Transport endpoint is not connected",
            ECONNREFUSED => "Connection refused",
            _ => "Unknown error",
        };
        write!(f, "{}", explain)
    }
}

impl From<ZxError> for SysError {
    fn from(e: ZxError) -> Self {
        match e {
            ZxError::INVALID_ARGS => SysError::EINVAL,
            ZxError::NOT_SUPPORTED => SysError::ENOSYS,
            ZxError::ALREADY_EXISTS => SysError::EEXIST,
            ZxError::SHOULD_WAIT => SysError::EAGAIN,
            ZxError::PEER_CLOSED => SysError::EPIPE,
            ZxError::BAD_HANDLE => SysError::EBADF,
            _ => unimplemented!("unknown error type convertion"),
        }
    }
}

impl From<FsError> for SysError {
    fn from(error: FsError) -> Self {
        match error {
            FsError::NotSupported => SysError::ENOSYS,
            FsError::NotFile => SysError::EISDIR,
            FsError::IsDir => SysError::EISDIR,
            FsError::NotDir => SysError::ENOTDIR,
            FsError::EntryNotFound => SysError::ENOENT,
            FsError::EntryExist => SysError::EEXIST,
            FsError::NotSameFs => SysError::EXDEV,
            FsError::InvalidParam => SysError::EINVAL,
            FsError::NoDeviceSpace => SysError::ENOMEM,
            FsError::DirRemoved => SysError::ENOENT,
            FsError::DirNotEmpty => SysError::ENOTEMPTY,
            FsError::WrongFs => SysError::EINVAL,
            FsError::DeviceError => SysError::EIO,
            FsError::IOCTLError => SysError::EINVAL,
            FsError::NoDevice => SysError::EINVAL,
            FsError::Again => SysError::EAGAIN,
            FsError::SymLoop => SysError::ELOOP,
            FsError::Busy => SysError::EBUSY,
        }
    }
}
