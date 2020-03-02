(function() {var implementors = {};
implementors["kernel_hal"] = [{"text":"impl&lt;T, P:&nbsp;<a class=\"trait\" href=\"kernel_hal/user/trait.Policy.html\" title=\"trait kernel_hal::user::Policy\">Policy</a>&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;usize&gt; for <a class=\"struct\" href=\"kernel_hal/user/struct.UserPtr.html\" title=\"struct kernel_hal::user::UserPtr\">UserPtr</a>&lt;T, P&gt;","synthetic":false,"types":["kernel_hal::user::UserPtr"]}];
implementors["linux_object"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"enum\" href=\"zircon_object/error/enum.ZxError.html\" title=\"enum zircon_object::error::ZxError\">ZxError</a>&gt; for <a class=\"enum\" href=\"linux_object/error/enum.LxError.html\" title=\"enum linux_object::error::LxError\">LxError</a>","synthetic":false,"types":["linux_object::error::LxError"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"enum\" href=\"linux_object/fs/vfs/enum.FsError.html\" title=\"enum linux_object::fs::vfs::FsError\">FsError</a>&gt; for <a class=\"enum\" href=\"linux_object/error/enum.LxError.html\" title=\"enum linux_object::error::LxError\">LxError</a>","synthetic":false,"types":["linux_object::error::LxError"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"enum\" href=\"kernel_hal/user/enum.Error.html\" title=\"enum kernel_hal::user::Error\">Error</a>&gt; for <a class=\"enum\" href=\"linux_object/error/enum.LxError.html\" title=\"enum linux_object::error::LxError\">LxError</a>","synthetic":false,"types":["linux_object::error::LxError"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.usize.html\">usize</a>&gt; for <a class=\"struct\" href=\"linux_object/fs/struct.FileDesc.html\" title=\"struct linux_object::fs::FileDesc\">FileDesc</a>","synthetic":false,"types":["linux_object::fs::FileDesc"]}];
implementors["zircon_object"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"enum\" href=\"kernel_hal/user/enum.Error.html\" title=\"enum kernel_hal::user::Error\">Error</a>&gt; for <a class=\"enum\" href=\"zircon_object/enum.ZxError.html\" title=\"enum zircon_object::ZxError\">ZxError</a>","synthetic":false,"types":["zircon_object::error::ZxError"]}];
implementors["zircon_syscall"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u32.html\">u32</a>&gt; for <a class=\"enum\" href=\"zircon_syscall/enum.SyscallType.html\" title=\"enum zircon_syscall::SyscallType\">SyscallType</a>","synthetic":false,"types":["zircon_syscall::consts::SyscallType"]}];

            if (window.register_implementors) {
                window.register_implementors(implementors);
            } else {
                window.pending_implementors = implementors;
            }
        })()