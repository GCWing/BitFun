use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::datasource::{DataSource, DataSourceConfig, SourceHealth};
use crate::error::{Result, TaijiError};
use crate::types::tick::{RawTick, SourceId};

#[derive(Clone)]
struct SourceRoute {
    source_id: SourceId,
    #[allow(dead_code)]
    priority: u8, // 0 = highest
}

struct Backoff {
    current_delay: Duration,
    max_delay: Duration,
    attempts: u32,
    max_attempts: u32,
}

impl Backoff {
    fn new() -> Self {
        Self {
            current_delay: Duration::from_secs(2),
            max_delay: Duration::from_secs(60),
            attempts: 0,
            max_attempts: 10,
        }
    }

    fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= self.max_attempts {
            return None;
        }
        let delay = self.current_delay;
        self.current_delay = std::cmp::min(self.current_delay * 2, self.max_delay);
        self.attempts += 1;
        Some(delay)
    }

    fn reset(&mut self) {
        self.current_delay = Duration::from_secs(2);
        self.attempts = 0;
    }
}

pub struct DataSourceManager {
    sources: HashMap<SourceId, Box<dyn DataSource>>,
    routes: HashMap<String, Vec<SourceRoute>>, // instrument → priority-sorted routes
    health: HashMap<SourceId, SourceHealth>,
    backoffs: HashMap<SourceId, Backoff>,
    last_health_check: Instant,
    health_interval: Duration,
}

// DataSourceManager cannot derive Default because last_health_check: Instant
// has no meaningful default value (Instant doesn't implement Default).
#[allow(clippy::new_without_default)]
impl DataSourceManager {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
            routes: HashMap::new(),
            health: HashMap::new(),
            backoffs: HashMap::new(),
            last_health_check: Instant::now(),
            health_interval: Duration::from_secs(30),
        }
    }

    /// 注册数据源
    pub fn add_source(&mut self, id: SourceId, ds: Box<dyn DataSource>) {
        self.health.insert(id.clone(), SourceHealth::Healthy);
        self.backoffs.insert(id.clone(), Backoff::new());
        self.sources.insert(id, ds);
    }

    /// 为品种添加路由（按优先级排列）
    pub fn add_route(&mut self, instrument: &str, source_ids: Vec<&str>) {
        let routes: Vec<SourceRoute> = source_ids
            .iter()
            .enumerate()
            .map(|(i, &sid)| SourceRoute {
                source_id: sid.to_string(),
                priority: i as u8,
            })
            .collect();
        self.routes.insert(instrument.to_string(), routes);
    }

    /// 获取品种的下一个 tick——try 主源 → 失败 → try 备源
    pub fn next_tick(&mut self, instrument: &str) -> Result<Option<RawTick>> {
        let routes = self.routes.get(instrument).cloned();
        let routes = routes.as_ref().ok_or_else(|| {
            TaijiError::Config(format!("no route for instrument '{}'", instrument))
        })?;

        for route in routes {
            if let Some(ds) = self.sources.get_mut(&route.source_id) {
                match ds.next_raw() {
                    Ok(Some(tick)) => {
                        if let Some(b) = self.backoffs.get_mut(&route.source_id) {
                            b.reset();
                        }
                        return Ok(Some(tick));
                    }
                    Ok(None) => continue,
                    Err(_) => {
                        // source failed, try next
                        continue;
                    }
                }
            }
        }

        Err(TaijiError::AllSourcesDown(instrument.to_string()))
    }

    /// 健康检查——周期性运行
    pub fn health_check_all(&mut self) {
        if self.last_health_check.elapsed() < self.health_interval {
            return;
        }
        for (id, ds) in &self.sources {
            let health = ds.health_check();
            self.health.insert(id.clone(), health);
        }
        self.last_health_check = Instant::now();
    }

    /// 退避重连
    pub fn reconnect(&mut self, source_id: &str) -> Result<()> {
        if let Some(backoff) = self.backoffs.get_mut(source_id) {
            if let Some(delay) = backoff.next_delay() {
                std::thread::sleep(delay);
                if let Some(ds) = self.sources.get_mut(source_id) {
                    let config = DataSourceConfig {
                        type_name: ds.name().to_string(),
                        params: HashMap::new(),
                    };
                    ds.disconnect()?;
                    ds.connect(&config)?;
                    backoff.reset();
                    self.health
                        .insert(source_id.to_string(), SourceHealth::Healthy);
                    return Ok(());
                }
            }
        }
        Err(TaijiError::DataSource(format!(
            "reconnect failed for '{}'",
            source_id
        )))
    }

    pub fn source_health(&self, source_id: &str) -> Option<&SourceHealth> {
        self.health.get(source_id)
    }
}
