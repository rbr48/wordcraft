# Izhaan Intellect Office Suite · VPS Deployment Guide

This directory contains everything required to deploy the **Izhaan Intellect Office Suite Portal** and **WordCraft Web App** to your VPS.

- **Production Domain:** `https://office.izhaanintellect.fun/`
- **VPS IP:** `148.135.137.245`
- **Architecture:** Multi-product routing designed to host WordCraft today, and SheetCraft + SlideCraft in future slots on the same domain.

---

## Directory Architecture

```text
deploy/
├── portal/                          # Landing page at root /
│   ├── index.html                   # Professional Izhaan Intellect Office Portal
│   ├── styles.css                   # Glassmorphic responsive dark theme
│   ├── app.js                       # Interactive scripts, modal, drag & drop
│   └── assets/                      # Original vector SVG assets & icons
│       ├── logo.svg                 # Office Suite shield & monogram
│       ├── icon-word.svg            # WordCraft app icon
│       ├── icon-sheet.svg           # SheetCraft app icon
│       ├── icon-slide.svg           # SlideCraft app icon
│       ├── icon-ai.svg              # AI Copilot icon
│       └── icon-bangla.svg          # Bangla engine icon
├── apps/
│   ├── word/                        # WordCraft Web static WASM build
│   │   ├── index.html               # Canvas shell with floating portal link
│   │   ├── *.js                     # wasm-bindgen glue module
│   │   ├── *.wasm                   # Optimized WebAssembly binary (~13MB)
│   │   └── ...                      # Typeface OFL notices & headers
│   ├── sheet/                       # Future slot for SheetCraft
│   └── slide/                       # Future slot for SlideCraft
├── nginx/
│   └── office.izhaanintellect.fun.conf  # Production Nginx site block with SSL & WASM MIME
├── docker/
│   ├── docker-compose.yml           # Containerized deployment with certbot auto-renewal
│   └── nginx.conf                   # Container Nginx config
├── deploy-vps.sh                    # 1-command bash deployment script for Ubuntu/Debian/CentOS
├── deploy-from-windows.ps1          # 1-command remote deployment script from Windows via SCP/SSH
└── README.md                        # This document
```

---

## URL & Sub-path Routing

| URL Path | Target Application | Filesystem Location on VPS |
|---|---|---|
| `https://office.izhaanintellect.fun/` | **Office Portal Landing Page** | `/var/www/office/portal/` |
| `https://office.izhaanintellect.fun/word/` | **WordCraft Web App (Live)** | `/var/www/office/apps/word/` |
| `https://office.izhaanintellect.fun/sheet/` | **SheetCraft (Future Slot)** | `/var/www/office/apps/sheet/` |
| `https://office.izhaanintellect.fun/slide/` | **SlideCraft (Future Slot)** | `/var/www/office/apps/slide/` |

---

## Quick Deployment (Choose Any Option)

### Option 1: One-Command Remote Deploy from Windows (Recommended)

From PowerShell on your development machine:

```powershell
.\deploy\deploy-from-windows.ps1 -VpsHost "148.135.137.245" -VpsUser "root"
```

This will automatically copy the `deploy/` directory to the server via SCP, make `deploy-vps.sh` executable, and execute it over SSH to configure Nginx, directories, and Let's Encrypt SSL.

---

### Option 2: Run Directly on the VPS via SSH

1. SSH into your VPS:
   ```bash
   ssh root@148.135.137.245
   ```

2. Copy or clone the repository/deploy folder, then run:
   ```bash
   cd /path/to/deploy
   chmod +x deploy-vps.sh
   sudo ./deploy-vps.sh
   ```

The script will:
- Install `nginx` and `certbot` if not already installed.
- Set up `/var/www/office/` structure.
- Copy portal files and WordCraft Web WASM files.
- Install and validate the Nginx configuration.
- Automatically obtain and configure a Let's Encrypt SSL certificate for `office.izhaanintellect.fun`.

---

### Option 3: Docker Compose

If you use Docker on the VPS:

```bash
cd deploy/docker
docker compose up -d
```

---

## DNS Requirement

Ensure your domain's DNS is configured with an **A Record**:
- **Host / Name:** `office` (or `office.izhaanintellect.fun`)
- **Type:** `A`
- **Value / Destination:** `148.135.137.245`
- **TTL:** `Auto` or `300`

---

## Testing Locally Before Deploying

You can test the entire portal and web app locally using Python:

```bash
cd deploy
python -m http.server 8080
```

Then visit:
- Portal: `http://localhost:8080/portal/`
- WordCraft Web: `http://localhost:8080/apps/word/`
