# Lab 1: Appetizer

---

## Information

Name: 李天宇

Email: 2200013188@stu.pku.edu.cn

参考了[Rust 程序设计语言](https://kaisery.github.io/trpl-zh-cn/)学习rust语言。

## Booting Tacos

A1: 

![Tacos](./Tacos_running_example.png)

## Debugging

### First instruction

B1: The first instruction that gets executed is `la t0, entry_pgtable` under the `_entry` label.

B2: 0x80200000, as shown in `boot.rs`

### From ZSBL to SBI

B3: `0xFFFFFFC080000000`

### SBI, kernel and argument passing

B4: 
- `hard_id`: `0`
- `dtb`: `0x82200000(2183135232)`

B5: 
- `Domain0 Next Address`: `0x0000000080200000`
- `Domain0 Next Arg1`: `0x0000000082200000`
- `Domain0 Next Mode`: `S-mode`
- `Boot HART ID`: `0`

B6: 
- `Domain0 Next Arg1` = `dtb`
- `Boot HART ID` = `hard_id`

(we know that `riscv` has 3 modes: `M-mode`(Machine Mode), `S-mode`(Supervisor Mode) and `U-mode`(User Mode))


### SBI interfaces

B7:
- `a7` stores `EID` of `SBI`, which equals `CONSOLE_PUTCHAR = 0x01`, 1
- `a6` stores `fid = 0`, 0

## Kernel Monitor

C1:

![Shell](./shell.png)

C2:
Read user's input by sbi function `console_getchar()`.
I found a `read()` function in `io.rs` but don't know how to use it.
So I decided to implement like this.