# URL 代理服务

一个基于 Rust 和 Axum 构建的简单 HTTP 代理服务，使用密码保护访问。

## 功能特性

- 🔐 密码保护访问
- 🚀 高性能 Rust 实现
- 🐳 Docker 容器化部署
- 📦 静态链接二进制文件
- 🔄 自动跟随重定向

## 快速开始

### 环境变量

- `PASSWORD`: 必需，用于访问验证的密码

### 使用 Docker 运行

```bash
docker run -d \
  -p 3000:3000 \
  -e PASSWORD=your_secret_password \
  ghcr.io/zhpjy/url-proxy:latest
```

### 本地运行

```bash
# 设置环境变量
export PASSWORD=your_secret_password

# 运行
./url-proxy
```

## 使用方法

启动服务后，通过以下格式访问目标 URL：

```
http://localhost:3000/{PASSWORD}/{目标URL}
```

例如：
- 访问 `http://example.com`:  `http://localhost:3000/your_secret_password/http://example.com`
- 访问 `https://example.com`: `http://localhost:3000/your_secret_password/https://example.com` 或者 `http://localhost:3000/your_secret_password/example.com`
- 访问 `https://api.github.com`: `http://localhost:3000/your_secret_password/https://api.github.com` 或者 `http://localhost:3000/your_secret_password/api.github.com`
          
## 许可证

MIT License