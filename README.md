# rust-staticlib-linker
link rust staticlib into freestanding shared library

# disclaimer

While this tool can be used to create a freestanding C dynamic shared object dso using musl toolchain, it is not useful because `musl` and `glibc` thread and TLS implementation are totally different, so most certainly using the produced dso would end up crash your program.
