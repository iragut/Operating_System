rustc \
    --edition 2021 \
    --target x86_64-unknown-none \
    -C linker=rust-lld \
    -C link-arg=-T -C link-arg=user/linker.ld \
    -C link-arg=--oformat=binary \
    -C relocation-model=static \
    -C panic=abort \
    user/echo.rs \
    -o user/echo.bin