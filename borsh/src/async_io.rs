use async_trait::async_trait;

#[async_trait]
pub trait AsyncRead: Unpin + Send {
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>;

    async fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()>;
}

#[cfg(feature = "tokio")]
#[async_trait]
impl<R: tokio::io::AsyncReadExt + Unpin + Send> AsyncRead for R {
    #[inline]
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        tokio::io::AsyncReadExt::read(self, buf).await
    }

    #[inline]
    async fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        tokio::io::AsyncReadExt::read_exact(self, buf)
            .await
            .map(|_| ())
    }
}

#[cfg(feature = "async-std")]
#[async_trait]
impl<R: async_std::io::ReadExt + Unpin + Send> AsyncRead for R {
    #[inline]
    async fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        async_std::io::ReadExt::read(self, buf).await
    }

    #[inline]
    async fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        async_std::io::ReadExt::read_exact(self, buf).await
    }
}

#[async_trait]
pub trait AsyncWrite: Unpin + Send {
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()>;
}

#[cfg(feature = "tokio")]
#[async_trait]
impl<R: tokio::io::AsyncWriteExt + Unpin + Send> AsyncWrite for R {
    #[inline]
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        tokio::io::AsyncWriteExt::write_all(self, buf).await
    }
}

#[cfg(feature = "async-std")]
#[async_trait]
impl<R: async_std::io::WriteExt + Unpin + Send> AsyncWrite for R {
    #[inline]
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        async_std::io::WriteExt::write_all(self, buf).await
    }
}