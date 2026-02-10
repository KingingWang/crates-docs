//! 工具函数模块

use crate::error::{Error, Result};
use reqwest::Client;
use std::time::Duration;
use tokio::sync::Semaphore;
use std::sync::Arc;

/// HTTP 客户端构建器
pub struct HttpClientBuilder {
    timeout: Duration,
    connect_timeout: Duration,
    pool_max_idle_per_host: usize,
    user_agent: String,
    enable_gzip: bool,
    enable_brotli: bool,
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            pool_max_idle_per_host: 10,
            user_agent: format!("CratesDocsMCP/{}", crate::VERSION),
            enable_gzip: true,
            enable_brotli: true,
        }
    }
}

impl HttpClientBuilder {
    /// 创建新的 HTTP 客户端构建器
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 设置请求超时
    #[must_use] 
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    /// 设置连接超时
    #[must_use] 
    pub fn connect_timeout(mut self, connect_timeout: Duration) -> Self {
        self.connect_timeout = connect_timeout;
        self
    }
    
    /// 设置连接池大小
    #[must_use] 
    pub fn pool_max_idle_per_host(mut self, max_idle: usize) -> Self {
        self.pool_max_idle_per_host = max_idle;
        self
    }
    
    /// 设置 User-Agent
    #[must_use] 
    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.user_agent = user_agent;
        self
    }
    
    /// 启用/禁用 Gzip 压缩
    #[must_use] 
    pub fn enable_gzip(mut self, enable: bool) -> Self {
        self.enable_gzip = enable;
        self
    }
    
    /// 启用/禁用 Brotli 压缩
    #[must_use] 
    pub fn enable_brotli(mut self, enable: bool) -> Self {
        self.enable_brotli = enable;
        self
    }
    
    /// 构建 HTTP 客户端
    pub fn build(self) -> Result<Client> {
        let mut builder = Client::builder()
            .timeout(self.timeout)
            .connect_timeout(self.connect_timeout)
            .pool_max_idle_per_host(self.pool_max_idle_per_host)
            .user_agent(&self.user_agent);
        
        // reqwest 0.13 默认启用 gzip 和 brotli
        // 如果需要禁用，可以使用 .no_gzip() 和 .no_brotli()
        if !self.enable_gzip {
            builder = builder.no_gzip();
        }
        
        if !self.enable_brotli {
            builder = builder.no_brotli();
        }
        
        builder.build()
            .map_err(|e| Error::HttpRequest(e.to_string()))
    }
}

/// 速率限制器
pub struct RateLimiter {
    semaphore: Arc<Semaphore>,
    max_permits: usize,
}

impl RateLimiter {
    /// 创建新的速率限制器
    #[must_use] 
    pub fn new(max_permits: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_permits)),
            max_permits,
        }
    }
    
    /// 获取许可（阻塞直到可用）
    pub async fn acquire(&self) -> Result<tokio::sync::SemaphorePermit<'_>> {
        self.semaphore.acquire().await
            .map_err(|e| Error::Other(format!("获取速率限制许可失败: {e}")))
    }
    
    /// 尝试获取许可（非阻塞）
    #[must_use] 
    pub fn try_acquire(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        self.semaphore.try_acquire().ok()
    }
    
    /// 获取当前可用许可数
    #[must_use] 
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
    
    /// 获取最大许可数
    #[must_use] 
    pub fn max_permits(&self) -> usize {
        self.max_permits
    }
}

/// 响应压缩工具
pub mod compression {
    use crate::error::{Error, Result};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;
    
    /// 压缩数据（Gzip）
    pub fn gzip_compress(data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)
            .map_err(|e| Error::Other(format!("Gzip 压缩失败: {e}")))?;
        encoder.finish()
            .map_err(|e| Error::Other(format!("Gzip 压缩完成失败: {e}")))
    }
    
    /// 解压数据（Gzip）
    pub fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = flate2::read::GzDecoder::new(data);
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed)
            .map_err(|e| Error::Other(format!("Gzip 解压失败: {e}")))?;
        Ok(decompressed)
    }
}

/// 字符串工具
pub mod string {
    /// 截断字符串并添加省略号
    #[must_use] 
    pub fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            return s.to_string();
        }
        
        if max_len <= 3 {
            return "...".to_string();
        }
        
        format!("{}...", &s[..max_len - 3])
    }
    
    /// 安全地解析数字
    pub fn parse_number<T: std::str::FromStr>(s: &str, default: T) -> T {
        s.parse().unwrap_or(default)
    }
    
    /// 检查字符串是否为空或空白
    #[must_use] 
    pub fn is_blank(s: &str) -> bool {
        s.trim().is_empty()
    }
}

/// 时间工具
pub mod time {
    use chrono::{DateTime, Utc};
    
    /// 获取当前时间戳（毫秒）
    #[must_use] 
    pub fn current_timestamp_ms() -> i64 {
        Utc::now().timestamp_millis()
    }
    
    /// 格式化时间
    #[must_use] 
    pub fn format_datetime(dt: &DateTime<Utc>) -> String {
        dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string()
    }
    
    /// 计算时间间隔（毫秒）
    #[must_use] 
    pub fn elapsed_ms(start: std::time::Instant) -> u128 {
        start.elapsed().as_millis()
    }
}

/// 验证工具
pub mod validation {
    use crate::error::Error;
    
    /// 验证 crate 名称
    pub fn validate_crate_name(name: &str) -> Result<(), Error> {
        if name.is_empty() {
            return Err(Error::Other("crate 名称不能为空".to_string()));
        }
        
        if name.len() > 100 {
            return Err(Error::Other("crate 名称过长".to_string()));
        }
        
        // 基本验证：只允许字母、数字、下划线、连字符
        if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(Error::Other("crate 名称包含无效字符".to_string()));
        }
        
        Ok(())
    }
    
    /// 验证版本号
    pub fn validate_version(version: &str) -> Result<(), Error> {
        if version.is_empty() {
            return Err(Error::Other("版本号不能为空".to_string()));
        }
        
        if version.len() > 50 {
            return Err(Error::Other("版本号过长".to_string()));
        }
        
        // 简单验证：应该包含数字和点
        if !version.chars().any(|c| c.is_ascii_digit()) {
            return Err(Error::Other("版本号必须包含数字".to_string()));
        }
        
        Ok(())
    }
    
    /// 验证搜索查询
    pub fn validate_search_query(query: &str) -> Result<(), Error> {
        if query.is_empty() {
            return Err(Error::Other("搜索查询不能为空".to_string()));
        }
        
        if query.len() > 200 {
            return Err(Error::Other("搜索查询过长".to_string()));
        }
        
        Ok(())
    }
}

/// 性能监控
pub mod metrics {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Instant;
    
    /// 性能计数器
    #[derive(Clone)]
    pub struct PerformanceCounter {
        total_requests: Arc<AtomicU64>,
        successful_requests: Arc<AtomicU64>,
        failed_requests: Arc<AtomicU64>,
        total_response_time_ms: Arc<AtomicU64>,
    }
    
    impl PerformanceCounter {
        /// 创建新的性能计数器
        #[must_use]
        pub fn new() -> Self {
            Self {
                total_requests: Arc::new(AtomicU64::new(0)),
                successful_requests: Arc::new(AtomicU64::new(0)),
                failed_requests: Arc::new(AtomicU64::new(0)),
                total_response_time_ms: Arc::new(AtomicU64::new(0)),
            }
        }
        
        /// 记录请求开始
        #[must_use] 
        pub fn record_request_start(&self) -> Instant {
            self.total_requests.fetch_add(1, Ordering::Relaxed);
            Instant::now()
        }
        
        /// 记录请求完成
        #[allow(clippy::cast_possible_truncation)]
        pub fn record_request_complete(&self, start: Instant, success: bool) {
            let duration_ms = start.elapsed().as_millis() as u64;
            self.total_response_time_ms.fetch_add(duration_ms, Ordering::Relaxed);
            
            if success {
                self.successful_requests.fetch_add(1, Ordering::Relaxed);
            } else {
                self.failed_requests.fetch_add(1, Ordering::Relaxed);
            }
        }
        
        /// 获取统计信息
        #[must_use] 
        pub fn get_stats(&self) -> PerformanceStats {
            let total = self.total_requests.load(Ordering::Relaxed);
            let success = self.successful_requests.load(Ordering::Relaxed);
            let failed = self.failed_requests.load(Ordering::Relaxed);
            let total_time = self.total_response_time_ms.load(Ordering::Relaxed);
            
            #[allow(clippy::cast_precision_loss)]
            let avg_response_time = if total > 0 {
                total_time as f64 / total as f64
            } else {
                0.0
            };
            
            #[allow(clippy::cast_precision_loss)]
            let success_rate = if total > 0 {
                success as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            
            PerformanceStats {
                total_requests: total,
                successful_requests: success,
                failed_requests: failed,
                average_response_time_ms: avg_response_time,
                success_rate_percent: success_rate,
            }
        }
        
        /// 重置计数器
        pub fn reset(&self) {
            self.total_requests.store(0, Ordering::Relaxed);
            self.successful_requests.store(0, Ordering::Relaxed);
            self.failed_requests.store(0, Ordering::Relaxed);
            self.total_response_time_ms.store(0, Ordering::Relaxed);
        }
    }
    
    impl Default for PerformanceCounter {
        fn default() -> Self {
            Self::new()
        }
    }
    
    /// 性能统计信息
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct PerformanceStats {
        /// 总请求数
        pub total_requests: u64,
        /// 成功请求数
        pub successful_requests: u64,
        /// 失败请求数
        pub failed_requests: u64,
        /// 平均响应时间（毫秒）
        pub average_response_time_ms: f64,
        /// 成功率（百分比）
        pub success_rate_percent: f64,
    }
}