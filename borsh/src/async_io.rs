use core::future::Future;

use crate::io::Result as BorshIoResult;

/// Asynchronous read trait.
///
/// `read_` methods imply little-endian byte order,
/// otherwise it's incorrect in the context of `borsh`.
///
/// Blanked implementations for `tokio` and `async-std` are provided.
pub trait AsyncRead: Unpin + Send {
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = BorshIoResult<usize>> + Send + 'a;

    fn read_exact<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = BorshIoResult<()>> + Send + 'a {
        async {
            let mut offset = 0;
            while offset < buf.len() {
                let read = self.read(&mut buf[offset..]).await?;
                if read == 0 {
                    return Err(crate::io::Error::new(
                        crate::io::ErrorKind::UnexpectedEof,
                        "failed to fill the whole buffer",
                    ));
                }
                offset += read;
            }
            Ok(())
        }
    }

    fn read_u8(&mut self) -> impl Future<Output = BorshIoResult<u8>> + Send {
        async {
            let mut buf = [0u8; 1];
            self.read_exact(&mut buf).await?;
            Ok(buf[0])
        }
    }

    fn read_u16(&mut self) -> impl Future<Output = BorshIoResult<u16>> + Send {
        async {
            let mut buf = [0u8; 2];
            self.read_exact(&mut buf).await?;
            Ok(u16::from_le_bytes(buf))
        }
    }

    fn read_u32(&mut self) -> impl Future<Output = BorshIoResult<u32>> + Send {
        async {
            let mut buf = [0u8; 4];
            self.read_exact(&mut buf).await?;
            Ok(u32::from_le_bytes(buf))
        }
    }

    fn read_u64(&mut self) -> impl Future<Output = BorshIoResult<u64>> + Send {
        async {
            let mut buf = [0u8; 8];
            self.read_exact(&mut buf).await?;
            Ok(u64::from_le_bytes(buf))
        }
    }

    fn read_u128(&mut self) -> impl Future<Output = BorshIoResult<u128>> + Send {
        async {
            let mut buf = [0u8; 16];
            self.read_exact(&mut buf).await?;
            Ok(u128::from_le_bytes(buf))
        }
    }

    fn read_i8(&mut self) -> impl Future<Output = BorshIoResult<i8>> + Send {
        async {
            let mut buf = [0u8; 1];
            self.read_exact(&mut buf).await?;
            Ok(buf[0] as i8)
        }
    }

    fn read_i16(&mut self) -> impl Future<Output = BorshIoResult<i16>> + Send {
        async {
            let mut buf = [0u8; 2];
            self.read_exact(&mut buf).await?;
            Ok(i16::from_le_bytes(buf))
        }
    }

    fn read_i32(&mut self) -> impl Future<Output = BorshIoResult<i32>> + Send {
        async {
            let mut buf = [0u8; 4];
            self.read_exact(&mut buf).await?;
            Ok(i32::from_le_bytes(buf))
        }
    }

    fn read_i64(&mut self) -> impl Future<Output = BorshIoResult<i64>> + Send {
        async {
            let mut buf = [0u8; 8];
            self.read_exact(&mut buf).await?;
            Ok(i64::from_le_bytes(buf))
        }
    }

    fn read_i128(&mut self) -> impl Future<Output = BorshIoResult<i128>> + Send {
        async {
            let mut buf = [0u8; 16];
            self.read_exact(&mut buf).await?;
            Ok(i128::from_le_bytes(buf))
        }
    }

    fn read_f32(&mut self) -> impl Future<Output = BorshIoResult<f32>> + Send {
        async {
            let mut buf = [0u8; 4];
            self.read_exact(&mut buf).await?;
            Ok(f32::from_le_bytes(buf))
        }
    }

    fn read_f64(&mut self) -> impl Future<Output = BorshIoResult<f64>> + Send {
        async {
            let mut buf = [0u8; 8];
            self.read_exact(&mut buf).await?;
            Ok(f64::from_le_bytes(buf))
        }
    }
}

#[cfg(feature = "unstable__tokio")]
impl<R: tokio::io::AsyncReadExt + Unpin + Send> AsyncRead for R {
    #[inline]
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = BorshIoResult<usize>> + Send + 'a {
        tokio::io::AsyncReadExt::read(self, buf)
    }

    #[inline]
    async fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> BorshIoResult<()> {
        tokio::io::AsyncReadExt::read_exact(self, buf)
            .await
            .map(|_| ())
    }

    #[inline]
    fn read_u8(&mut self) -> impl Future<Output = BorshIoResult<u8>> + Send {
        tokio::io::AsyncReadExt::read_u8(self)
    }

    #[inline]
    fn read_u16(&mut self) -> impl Future<Output = BorshIoResult<u16>> + Send {
        tokio::io::AsyncReadExt::read_u16_le(self)
    }

    #[inline]
    fn read_u32(&mut self) -> impl Future<Output = BorshIoResult<u32>> + Send {
        tokio::io::AsyncReadExt::read_u32_le(self)
    }

    #[inline]
    fn read_u64(&mut self) -> impl Future<Output = BorshIoResult<u64>> + Send {
        tokio::io::AsyncReadExt::read_u64_le(self)
    }

    #[inline]
    fn read_u128(&mut self) -> impl Future<Output = BorshIoResult<u128>> + Send {
        tokio::io::AsyncReadExt::read_u128_le(self)
    }

    #[inline]
    fn read_i8(&mut self) -> impl Future<Output = BorshIoResult<i8>> + Send {
        tokio::io::AsyncReadExt::read_i8(self)
    }

    #[inline]
    fn read_i16(&mut self) -> impl Future<Output = BorshIoResult<i16>> + Send {
        tokio::io::AsyncReadExt::read_i16_le(self)
    }

    #[inline]
    fn read_i32(&mut self) -> impl Future<Output = BorshIoResult<i32>> + Send {
        tokio::io::AsyncReadExt::read_i32_le(self)
    }

    #[inline]
    fn read_i64(&mut self) -> impl Future<Output = BorshIoResult<i64>> + Send {
        tokio::io::AsyncReadExt::read_i64_le(self)
    }

    #[inline]
    fn read_i128(&mut self) -> impl Future<Output = BorshIoResult<i128>> + Send {
        tokio::io::AsyncReadExt::read_i128_le(self)
    }

    #[inline]
    fn read_f32(&mut self) -> impl Future<Output = BorshIoResult<f32>> + Send {
        tokio::io::AsyncReadExt::read_f32_le(self)
    }

    #[inline]
    fn read_f64(&mut self) -> impl Future<Output = BorshIoResult<f64>> + Send {
        tokio::io::AsyncReadExt::read_f64_le(self)
    }
}

#[cfg(feature = "unstable__async-std")]
impl<R: async_std::io::ReadExt + Unpin + Send> AsyncRead for R {
    #[inline]
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = BorshIoResult<usize>> + Send + 'a {
        async_std::io::ReadExt::read(self, buf)
    }

    #[inline]
    fn read_exact<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = BorshIoResult<()>> + Send + 'a {
        async_std::io::ReadExt::read_exact(self, buf)
    }
}

/// Asynchronous write trait.
///
/// `write_` methods imply little-endian byte order,
/// otherwise it's incorrect in the context of `borsh`.
///
/// Blanked implementations for `tokio` and `async-std` are provided.
pub trait AsyncWrite: Unpin + Send {
    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl Future<Output = BorshIoResult<()>> + Send + 'a;

    fn write_u8(&mut self, n: u8) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u16(&mut self, n: u16) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u32(&mut self, n: u32) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u64(&mut self, n: u64) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u128(&mut self, n: u128) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i8(&mut self, n: i8) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i16(&mut self, n: i16) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i32(&mut self, n: i32) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i64(&mut self, n: i64) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i128(&mut self, n: i128) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_f32(&mut self, n: f32) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_f64(&mut self, n: f64) -> impl Future<Output = BorshIoResult<()>> + Send {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }
}

#[cfg(feature = "unstable__tokio")]
impl<R: tokio::io::AsyncWriteExt + Unpin + Send> AsyncWrite for R {
    #[inline]
    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl Future<Output = BorshIoResult<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_all(self, buf)
    }

    #[inline]
    fn write_u8(&mut self, n: u8) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_u8(self, n)
    }

    #[inline]
    fn write_u16(&mut self, n: u16) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_u16_le(self, n)
    }

    #[inline]
    fn write_u32(&mut self, n: u32) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_u32_le(self, n)
    }

    #[inline]
    fn write_u64(&mut self, n: u64) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_u64_le(self, n)
    }

    #[inline]
    fn write_u128(&mut self, n: u128) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_u128_le(self, n)
    }

    #[inline]
    fn write_i8(&mut self, n: i8) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_i8(self, n)
    }

    #[inline]
    fn write_i16(&mut self, n: i16) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_i16_le(self, n)
    }

    #[inline]
    fn write_i32(&mut self, n: i32) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_i32_le(self, n)
    }

    #[inline]
    fn write_i64(&mut self, n: i64) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_i64_le(self, n)
    }

    #[inline]
    fn write_i128(&mut self, n: i128) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_i128_le(self, n)
    }

    #[inline]
    fn write_f32(&mut self, n: f32) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_f32_le(self, n)
    }

    #[inline]
    fn write_f64(&mut self, n: f64) -> impl Future<Output = BorshIoResult<()>> + Send {
        tokio::io::AsyncWriteExt::write_f64_le(self, n)
    }
}

#[cfg(feature = "unstable__async-std")]
impl<R: async_std::io::WriteExt + Unpin + Send> AsyncWrite for R {
    #[inline]
    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl Future<Output = BorshIoResult<()>> + Send + 'a {
        async_std::io::WriteExt::write_all(self, buf)
    }
}
