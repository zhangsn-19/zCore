# OSZcoreArm

## os大实验小组文档

> OS-F-3 张书宁 王星淇 徐晨曦

### 一. 项目目标

在树莓派(Raspberry Pi 4b) 板上运行 zCore (Armv8a, CortexA72)，并支持 zCore/zircon 下安全的若干应用。

#### 阶段性目标

0. 完成小实验 (2021Autumn rCore/zCore)

1. 在 Riscv/x86 平台上运行 zCore/rCore/其他相关操作系统仓库，感受 rust 编写的不同操作系统在不同平台上的驱动、调用、中断、堆栈和地址安排等实现方式。
2. 在 Qemu-arm 平台上运行 rCore/其他相关操作系统仓库，感受 rust 如何在 ARM 平台上运行，相关驱动和地址安排如何规定等。
3. 在 Qemu-arm 平台上适配 zCore。具体的，我们需要修改包括 kernel-hal, config 文件在内的一系列底层实现，使原本的 zCore 框架能够适配在 ARM 平台上，这里的检验方式是使其能够在 Qemu-arm 平台上运行。
4. 在树莓派（Raspberry Pi 4b) 板上适配 zCore。具体的，这里和 Qemu-arm 平台应该较为相似，但可能会由于硬件和版本原因有一定具体实现上的不同，可能在物理板上还需要一定的调试。
5. 在树莓派（Raspberry Pi 4b) 板上运行尽可能多的应用，根据应用的不同，需求的文件系统和系统调用的不同可能会需要向现有的 ARM 版本 zCore 中支持更多的 feature和功能。

#### 当前进度

- 阅读了 zCore 的 Aarch 部分参考驱动和参考代码，ARM的相关文档，进行了其他文献调研。
- 在 x86 平台完成了zCore 运行
- 构建出了QEMU的ARM虚拟机并成功跑通了前人的工作 (NimbOS, https://github.com/rvm-rtos/nimbos)
- 正在将小实验 rCore 移植到ARM平台
- 正在调试 zCore 在ARM平台的驱动部分
- 正在确认树莓派对应的 ARMv8 对驱动和系统调用的规定和需求

#### 参考文档和实现帮助

rCore https://github.com/rcore-os/rCore

zCore https://rcore-os.github.io/zCore-Tutorial/zcore-intro.html

zCore 教程 https://rcore-os.github.io/zCore-Tutorial/zcore-intro.html

rCore 教程 https://rcore-os.github.io/rCore-Tutorial-Book-v3/index.html

nimbOS  https://github.com/rvm-rtos/nimbos

#### 小组分工

##### 张书宁

###### 本阶段

###### 下一阶段

完成 zCore 驱动部分的 rust 实现和调试。

##### 王星淇

###### 本阶段

###### 下一阶段

##### 徐晨曦

###### 本阶段

###### 下一阶段



#### 鸣谢

感谢清华大学计算机系操作系统课程的陈渝，向勇，李国良等老师和张译仁，贾越凯，安之达等同学的帮助和支持。