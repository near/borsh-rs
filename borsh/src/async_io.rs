use core::future::Future;
use std::io::Result;

// use async_trait::async_trait;

// #[async_trait]
pub trait AsyncRead: Unpin + Send {
    fn read<'a>(&'a mut self, buf: &'a mut [u8])
        -> impl Future<Output = Result<usize>> + Send + 'a;

    fn read_exact<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<()>> + Send + 'a;

    fn read_u8<'a>(&'a mut self) -> impl Future<Output = Result<u8>> + Send + 'a {
        async {
            let mut buf = [0u8; 1];
            self.read_exact(&mut buf).await?;
            Ok(buf[0])
        }
    }

    fn read_u16<'a>(&'a mut self) -> impl Future<Output = Result<u16>> + Send + 'a {
        async {
            let mut buf = [0u8; 2];
            self.read_exact(&mut buf).await?;
            Ok(u16::from_le_bytes(buf))
        }
    }

    fn read_u32<'a>(&'a mut self) -> impl Future<Output = Result<u32>> + Send + 'a {
        async {
            let mut buf = [0u8; 4];
            self.read_exact(&mut buf).await?;
            Ok(u32::from_le_bytes(buf))
        }
    }

    fn read_u64<'a>(&'a mut self) -> impl Future<Output = Result<u64>> + Send + 'a {
        async {
            let mut buf = [0u8; 8];
            self.read_exact(&mut buf).await?;
            Ok(u64::from_le_bytes(buf))
        }
    }

    fn read_u128<'a>(&'a mut self) -> impl Future<Output = Result<u128>> + Send + 'a {
        async {
            let mut buf = [0u8; 16];
            self.read_exact(&mut buf).await?;
            Ok(u128::from_le_bytes(buf))
        }
    }

    fn read_i8<'a>(&'a mut self) -> impl Future<Output = Result<i8>> + Send + 'a {
        async {
            let mut buf = [0u8; 1];
            self.read_exact(&mut buf).await?;
            Ok(buf[0] as i8)
        }
    }

    fn read_i16<'a>(&'a mut self) -> impl Future<Output = Result<i16>> + Send + 'a {
        async {
            let mut buf = [0u8; 2];
            self.read_exact(&mut buf).await?;
            Ok(i16::from_le_bytes(buf))
        }
    }

    fn read_i32<'a>(&'a mut self) -> impl Future<Output = Result<i32>> + Send + 'a {
        async {
            let mut buf = [0u8; 4];
            self.read_exact(&mut buf).await?;
            Ok(i32::from_le_bytes(buf))
        }
    }

    fn read_i64<'a>(&'a mut self) -> impl Future<Output = Result<i64>> + Send + 'a {
        async {
            let mut buf = [0u8; 8];
            self.read_exact(&mut buf).await?;
            Ok(i64::from_le_bytes(buf))
        }
    }

    fn read_i128<'a>(&'a mut self) -> impl Future<Output = Result<i128>> + Send + 'a {
        async {
            let mut buf = [0u8; 16];
            self.read_exact(&mut buf).await?;
            Ok(i128::from_le_bytes(buf))
        }
    }

    fn read_f32<'a>(&'a mut self) -> impl Future<Output = Result<f32>> + Send + 'a {
        async {
            let mut buf = [0u8; 4];
            self.read_exact(&mut buf).await?;
            Ok(f32::from_le_bytes(buf))
        }
    }

    fn read_f64<'a>(&'a mut self) -> impl Future<Output = Result<f64>> + Send + 'a {
        async {
            let mut buf = [0u8; 8];
            self.read_exact(&mut buf).await?;
            Ok(f64::from_le_bytes(buf))
        }
    }
}

#[cfg(feature = "tokio")]
// #[async_trait]
impl<R: tokio::io::AsyncReadExt + Unpin + Send> AsyncRead for R {
    #[inline]
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> impl Future<Output = Result<usize>> + 'a {
        tokio::io::AsyncReadExt::read(self, buf)
    }

    #[inline]
    fn read_exact<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<()>> + Send + 'a {
        async {
            tokio::io::AsyncReadExt::read_exact(self, buf)
                .await
                .map(|_| ())
        }
    }

    #[inline]
    fn read_u8<'a>(&'a mut self) -> impl Future<Output = Result<u8>> + Send + 'a {
        tokio::io::AsyncReadExt::read_u8(self)
    }

    #[inline]
    fn read_u16<'a>(&'a mut self) -> impl Future<Output = Result<u16>> + Send + 'a {
        tokio::io::AsyncReadExt::read_u16_le(self)
    }

    #[inline]
    fn read_u32<'a>(&'a mut self) -> impl Future<Output = Result<u32>> + Send + 'a {
        tokio::io::AsyncReadExt::read_u32_le(self)
    }

    #[inline]
    fn read_u64<'a>(&'a mut self) -> impl Future<Output = Result<u64>> + Send + 'a {
        tokio::io::AsyncReadExt::read_u64_le(self)
    }

    #[inline]
    fn read_u128<'a>(&'a mut self) -> impl Future<Output = Result<u128>> + Send + 'a {
        tokio::io::AsyncReadExt::read_u128_le(self)
    }

    #[inline]
    fn read_i8<'a>(&'a mut self) -> impl Future<Output = Result<i8>> + Send + 'a {
        tokio::io::AsyncReadExt::read_i8(self)
    }

    #[inline]
    fn read_i16<'a>(&'a mut self) -> impl Future<Output = Result<i16>> + Send + 'a {
        tokio::io::AsyncReadExt::read_i16_le(self)
    }

    #[inline]
    fn read_i32<'a>(&'a mut self) -> impl Future<Output = Result<i32>> + Send + 'a {
        tokio::io::AsyncReadExt::read_i32_le(self)
    }

    #[inline]
    fn read_i64<'a>(&'a mut self) -> impl Future<Output = Result<i64>> + Send + 'a {
        tokio::io::AsyncReadExt::read_i64_le(self)
    }

    #[inline]
    fn read_i128<'a>(&'a mut self) -> impl Future<Output = Result<i128>> + Send + 'a {
        tokio::io::AsyncReadExt::read_i128_le(self)
    }

    #[inline]
    fn read_f32<'a>(&'a mut self) -> impl Future<Output = Result<f32>> + Send + 'a {
        tokio::io::AsyncReadExt::read_f32_le(self)
    }

    #[inline]
    fn read_f64<'a>(&'a mut self) -> impl Future<Output = Result<f64>> + Send + 'a {
        tokio::io::AsyncReadExt::read_f64_le(self)
    }
}

#[cfg(feature = "async-std")]
// #[async_trait]
impl<R: async_std::io::ReadExt + Unpin + Send> AsyncRead for R {
    #[inline]
    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<usize>> + Send + 'a {
        async_std::io::ReadExt::read(self, buf)
    }

    #[inline]
    fn read_exact<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<()>> + Send + 'a {
        async_std::io::ReadExt::read_exact(self, buf)
    }
}

// #[async_trait]
pub trait AsyncWrite: Unpin + Send {
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> impl Future<Output = Result<()>> + Send + 'a;

    fn write_u8<'a>(&'a mut self, n: u8) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u16<'a>(&'a mut self, n: u16) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u32<'a>(&'a mut self, n: u32) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u64<'a>(&'a mut self, n: u64) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_u128<'a>(&'a mut self, n: u128) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i8<'a>(&'a mut self, n: i8) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i16<'a>(&'a mut self, n: i16) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i32<'a>(&'a mut self, n: i32) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i64<'a>(&'a mut self, n: i64) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_i128<'a>(&'a mut self, n: i128) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_f32<'a>(&'a mut self, n: f32) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }

    fn write_f64<'a>(&'a mut self, n: f64) -> impl Future<Output = Result<()>> + Send + 'a {
        async move {
            let bytes = n.to_le_bytes();
            self.write_all(&bytes).await?;
            Ok(())
        }
    }
}

#[cfg(feature = "tokio")]
// #[async_trait]
impl<R: tokio::io::AsyncWriteExt + Unpin + Send> AsyncWrite for R {
    #[inline]
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_all(self, buf)
    }

    #[inline]
    fn write_u8<'a>(&'a mut self, n: u8) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_u8(self, n)
    }

    #[inline]
    fn write_u16<'a>(&'a mut self, n: u16) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_u16_le(self, n)
    }

    #[inline]
    fn write_u32<'a>(&'a mut self, n: u32) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_u32_le(self, n)
    }

    #[inline]
    fn write_u64<'a>(&'a mut self, n: u64) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_u64_le(self, n)
    }

    #[inline]
    fn write_u128<'a>(&'a mut self, n: u128) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_u128_le(self, n)
    }

    #[inline]
    fn write_i8<'a>(&'a mut self, n: i8) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_i8(self, n)
    }

    #[inline]
    fn write_i16<'a>(&'a mut self, n: i16) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_i16_le(self, n)
    }

    #[inline]
    fn write_i32<'a>(&'a mut self, n: i32) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_i32_le(self, n)
    }

    #[inline]
    fn write_i64<'a>(&'a mut self, n: i64) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_i64_le(self, n)
    }

    #[inline]
    fn write_i128<'a>(&'a mut self, n: i128) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_i128_le(self, n)
    }

    #[inline]
    fn write_f32<'a>(&'a mut self, n: f32) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_f32_le(self, n)
    }

    #[inline]
    fn write_f64<'a>(&'a mut self, n: f64) -> impl Future<Output = Result<()>> + Send + 'a {
        tokio::io::AsyncWriteExt::write_f64_le(self, n)
    }
}

#[cfg(feature = "async-std")]
// #[async_trait]
impl<R: async_std::io::WriteExt + Unpin + Send> AsyncWrite for R {
    #[inline]
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> impl Future<Output = Result<()>> + Send + 'a {
        async_std::io::WriteExt::write_all(self, buf)
    }
}
