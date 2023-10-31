<img src="linos-logo.svg" alt="logo" width="100%">

A simple single binary linux distribution

The main goal is to avoid the complexity of maintaining
and patching a full-blown linux distribution when
running a simple single binary application on a
linux system (like rust/go/c ...).

*xn-μ3q* consists of the following components:

 - Your application
 - The [linux kernel](kernel.org)
 - The single binary linµos `/init` system.

   Needed to manage the boot, file system access,
   network configuration, updating, ...

   It uses the [libc](https://www.gnu.org/software/libc/) library to
   interact with the kernel.

## Development

To test *xn-μ3q* in a virtual environment, we build a linux kernel:

```
# inside the `linux` directory
make defconfig
make -j$(nproc)
```

Then we build and run the *xn-μ3q* system:

```
KERNEL=../linux/kernel make run
```
