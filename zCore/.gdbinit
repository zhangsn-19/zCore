set confirm off
set architecture riscv:rv64
target remote 127.0.0.1:15234
symbol-file ../target/riscv64/release/zcore
display/10i $pc
break *0x8020004a
break 
