use crossterm::{
    cursor::Show,
    event::DisableMouseCapture,
    execute,
    terminal::{disable_raw_mode, LeaveAlternateScreen},
};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::config::SSHConfig;
use crate::proxy::GlobalConfig;

#[derive(Debug, Clone)]
pub struct SSHManager {
    pub global_config: GlobalConfig,
}

impl SSHManager {
    /// 创建新的 SSH 管理器
    ///
    /// # 参数
    /// - `global_config`: 全局配置
    ///
    /// # 返回
    /// 返回初始化的 SSH 管理器
    pub fn new(global_config: GlobalConfig) -> Self {
        Self { global_config }
    }

    /// 建立 SSH 连接
    ///
    /// # 参数
    /// - `config`: SSH 配置
    ///
    /// # 返回
    /// 返回 Result，成功为 Ok(())，失败为 Err
    pub fn connect(&self, config: &SSHConfig) -> Result<(), Box<dyn std::error::Error>> {
        // 先恢复终端设置，退出 TUI 模式
        disable_raw_mode()?;
        execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture, // 禁用鼠标捕获
            Show                 // 显示光标
        )?;

        // 确保终端设置完全恢复
        std::io::stdout().flush()?;

        // 清屏并显示连接信息
        println!("\x1b[33m正在连接: {}\x1b[0m", config.address);

        // 显示代理信息
        if config.use_global_proxy && self.global_config.proxy.is_enabled() {
            println!(
                "\x1b[33m全局代理:\x1b[0m {:?} {}:{}",
                self.global_config.proxy.proxy_type,
                self.global_config.proxy.host,
                self.global_config.proxy.port.unwrap_or(
                    match self.global_config.proxy.proxy_type {
                        crate::proxy::ProxyType::Socks5 => 1080,
                        crate::proxy::ProxyType::Http => 8080,
                        crate::proxy::ProxyType::None => 0,
                    }
                )
            );
        } else if let Some(proxy) = &config.proxy {
            if proxy.is_enabled() {
                println!(
                    "\x1b[33m自定义代理:\x1b[0m {:?} {}:{}",
                    proxy.proxy_type,
                    proxy.host,
                    proxy.port.unwrap_or(match proxy.proxy_type {
                        crate::proxy::ProxyType::Socks5 => 1080,
                        crate::proxy::ProxyType::Http => 8080,
                        crate::proxy::ProxyType::None => 0,
                    })
                );
            }
        }

        // 显示端口转发信息
        if let Some(pf) = &config.port_forward {
            if pf.enabled {
                println!("\x1b[33m端口转发:\x1b[0m  {} <->  {}", pf.local, pf.remote);
            }
        }

        // 再次强制刷新
        std::io::stdout().flush()?;

        // 构建 SSH 命令
        let mut cmd = Command::new("ssh");

        // Add port
        if let Some(port) = config.port {
            cmd.arg("-p").arg(port.to_string());
        }

        // Add key
        if let Some(key) = &config.key {
            cmd.arg("-i").arg(key);
        }

        // Add proxy command
        if config.use_global_proxy {
            // 使用全局代理
            if let Some(proxy_cmd) = self.global_config.proxy.get_ssh_proxy_command() {
                cmd.arg("-o").arg(format!("ProxyCommand={proxy_cmd}"));
            }
        } else if let Some(proxy) = &config.proxy {
            // 使用自定义代理
            if let Some(proxy_cmd) = proxy.get_ssh_proxy_command() {
                cmd.arg("-o").arg(format!("ProxyCommand={proxy_cmd}"));
            }
        }

        // Add port forwarding
        if let Some(pf) = &config.port_forward {
            if pf.enabled {
                cmd.arg("-L").arg(format!("{}:{}", pf.local, pf.remote));
            }
        }

        // Build connection string
        let connection_string = if let Some(user) = &config.user {
            format!("{}@{}", user, config.address)
        } else {
            config.address.clone()
        };

        cmd.arg(connection_string);

        // 需要将cmd转义字符串，否则无法正确执行
        let cmd_str = std::iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(|s| {
                // 尝试将 OsStr 转为字符串，必要时加引号
                let s = s.to_string_lossy();
                if s.contains(' ') || s.contains('"') || s.contains('\'') {
                    format!("'{}'", s.replace('\'', "'\\''"))
                } else {
                    s.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        println!("\x1b[33m最终执行命令:\x1b[0m {cmd_str}");
        println!("按 Ctrl+C 取消连接\n");

        // Set standard streams
        cmd.stdin(std::process::Stdio::inherit());
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        // 使用 exec 替换当前进程
        let err = cmd.exec();

        // 如果 exec 失败，确保清理终端设置并显示光标
        disable_raw_mode()?;
        execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture, // 禁用鼠标捕获
            Show                 // 确保显示光标
        )?;
        std::io::stdout().flush()?;

        // If exec fails, return error
        Err(format!("执行失败: {err}").into())
    }
}
