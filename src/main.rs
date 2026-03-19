#![warn(missing_docs)]

//! 该项目整合网上免费的 SOCKS5, 并将 socks5 转为 socks5h.

use anyhow::Result;
use rust_tools::flexi_logger::init_flexi_logger;
use rust_tools::reqwest::get;
use std::collections::HashSet;
use std::{process::exit, time::Duration};
use tokio::{
    fs::write,
    runtime::{Builder, Runtime},
    signal,
};
use tokio_cron_scheduler::{Job, JobScheduler};

fn main() {
    let logger = init_flexi_logger("free-proxy-list", "./logs", "info");
    let logger = match logger {
        Ok(v) => v,
        Err(e) => {
            eprintln!("初始化日志失败: {e}");
            exit(1);
        }
    };

    let runtime = new_multi_thread().unwrap();
    runtime.block_on(async move {
        let scheduler = match JobScheduler::new().await {
            Ok(v) => v,
            Err(e) => {
                log::error!("创建 cron 调度器失败: {e}");
                logger.flush();
                logger.shutdown();
                return;
            }
        };
        let job = match Job::new_async("0 */15 * * * *", |_uuid, _lock| {
            Box::pin(async move {
                async_main().await;
            })
        }) {
            Ok(v) => v,
            Err(e) => {
                log::error!("创建 cron 任务失败: {e}");
                logger.flush();
                logger.shutdown();
                return;
            }
        };
        if let Err(e) = scheduler.add(job).await {
            log::error!("注册 cron 任务失败: {e}");
            logger.flush();
            logger.shutdown();
            return;
        }

        async_main().await;

        if let Err(e) = scheduler.start().await {
            log::error!("启动 cron 调度器失败: {e}");
            logger.flush();
            logger.shutdown();
            return;
        }

        signal::ctrl_c().await.unwrap();
        logger.flush();
        logger.shutdown();
    });
}

/// 异步执行入口
async fn async_main() {
    let mut text = String::new();

    let urls = vec![
        "https://raw.githubusercontent.com/dpangestuw/Free-Proxy/main/socks5_proxies.txt",
        "https://cdn.jsdelivr.net/gh/proxifly/free-proxy-list@main/proxies/protocols/socks5/data.txt",
    ];
    for url in urls {
        let r = fetch_proxy_text(url).await;
        match r {
            Ok(v) => text.push_str(&v),
            Err(e) => {
                log::error!("下载代理列表失败: {e}");
                return;
            }
        }
    }

    let text = dedupe_by_ip(&text);
    match write("socks5h.txt", text).await {
        Ok(_) => log::info!("代理列表已保存到 socks5h.txt"),
        Err(e) => log::error!("保存代理列表失败: {e}"),
    }
}

/// 下载代理列表并将 socks5 转为 socks5h.
async fn fetch_proxy_text(url: &str) -> Result<String> {
    log::info!("下载代理列表: {url}");

    let r = get(url).await?;
    let r = String::from_utf8(r.to_vec())?;
    let mut r = r.replace("socks5", "socks5h");
    if !r.ends_with('\n') {
        r.push('\n');
    }
    Ok(r)
}

/// 按 IP 去重，保留每个 IP 的第一条代理
fn dedupe_by_ip(text: &str) -> String {
    let mut seen_ips = HashSet::new();
    let mut result = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Some(addr) = line.strip_prefix("socks5h://") else {
            continue;
        };

        let Some((ip, _port)) = addr.rsplit_once(':') else {
            continue;
        };

        if seen_ips.insert(ip.to_string()) {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// 新建多线程运行时
#[allow(dead_code)]
fn new_multi_thread() -> Result<Runtime> {
    let mut builder = Builder::new_multi_thread();
    let builder = builder
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .worker_threads(5)
        .max_blocking_threads(5)
        .thread_keep_alive(Duration::from_secs(60));
    log::info!("创建多线程 Tokio 运行时:{builder:?}");
    Ok(builder.build()?)
}
