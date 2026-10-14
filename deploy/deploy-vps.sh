#!/usr/bin/env bash
# ==============================================================================
# Izhaan Intellect Office Suite — Automated VPS Deployment Script
# Target Domain: office.izhaanintellect.fun
# Target Host: 148.135.137.245
# ==============================================================================

set -euo pipefail

DOMAIN="office.izhaanintellect.fun"
WEB_ROOT="/var/www/office"
EMAIL="admin@izhaanintellect.fun"

echo "==================================================================="
echo "  Deploying Izhaan Intellect Office Portal & WordCraft Web App"
echo "  Domain: https://${DOMAIN}"
echo "==================================================================="

# 1. Require root or sudo
if [[ $EUID -ne 0 ]]; then
   echo "[!] This script must be run as root or with sudo."
   exit 1
fi

# 2. Detect OS and install Nginx + Certbot if missing
echo "[+] Step 1: Checking and installing Nginx & Certbot..."
if command -v apt-get &> /dev/null; then
    apt-get update -y
    apt-get install -y nginx certbot python3-certbot-nginx rsync ufw curl
elif command -v dnf &> /dev/null; then
    dnf install -y epel-release
    dnf install -y nginx certbot python3-certbot-nginx rsync curl
fi

# 3. Create web directories
echo "[+] Step 2: Preparing web directories at ${WEB_ROOT}..."
mkdir -p "${WEB_ROOT}/portal"
mkdir -p "${WEB_ROOT}/apps/word"
mkdir -p "${WEB_ROOT}/apps/sheet"
mkdir -p "${WEB_ROOT}/apps/slide"
mkdir -p "/var/www/certbot"

# 4. Copy files from script's relative directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Download official release bundle if apps/word wasm is missing
if ! ls "${SCRIPT_DIR}/apps/word/"*.wasm 1> /dev/null 2>&1; then
    echo "[*] WebAssembly binary not found locally in deploy/apps/word/."
    echo "[*] Downloading official WordCraft Web bundle from GitHub releases..."
    mkdir -p "${SCRIPT_DIR}/apps/word"
    TMP_ZIP="/tmp/wordcraft-web-v0.6.3.zip"
    curl -sSL -o "${TMP_ZIP}" "https://github.com/rbr48/wordcraft/releases/download/v0.6.3/wordcraft-web-0.6.3.zip"
    rm -rf /tmp/wordcraft-web-extracted
    mkdir -p /tmp/wordcraft-web-extracted
    unzip -q -o "${TMP_ZIP}" -d /tmp/wordcraft-web-extracted
    cp -r /tmp/wordcraft-web-extracted/wordcraft-web-*/* "${SCRIPT_DIR}/apps/word/"
fi

echo "[+] Step 3: Syncing Portal Landing Page to ${WEB_ROOT}/portal..."
rsync -av --delete "${SCRIPT_DIR}/portal/" "${WEB_ROOT}/portal/"

echo "[+] Step 4: Syncing WordCraft Web WASM to ${WEB_ROOT}/apps/word..."
rsync -av --delete "${SCRIPT_DIR}/apps/word/" "${WEB_ROOT}/apps/word/"

# 5. Fix ownership and permissions
echo "[+] Step 5: Setting web permissions..."
WEB_USER="www-data"
if ! id "www-data" &>/dev/null; then
    WEB_USER="nginx"
fi
chown -R ${WEB_USER}:${WEB_USER} "${WEB_ROOT}"
chmod -R 755 "${WEB_ROOT}"

# 6. Deploy Nginx Configuration
echo "[+] Step 6: Installing Nginx site configuration..."
NGINX_CONF_TARGET="/etc/nginx/sites-available/${DOMAIN}.conf"
NGINX_ENABLED_LINK="/etc/nginx/sites-enabled/${DOMAIN}.conf"

# Some distributions use conf.d directly
if [[ ! -d "/etc/nginx/sites-available" ]]; then
    NGINX_CONF_TARGET="/etc/nginx/conf.d/${DOMAIN}.conf"
    cp "${SCRIPT_DIR}/nginx/${DOMAIN}.conf" "${NGINX_CONF_TARGET}"
else
    cp "${SCRIPT_DIR}/nginx/${DOMAIN}.conf" "${NGINX_CONF_TARGET}"
    mkdir -p /etc/nginx/sites-enabled
    ln -sf "${NGINX_CONF_TARGET}" "${NGINX_ENABLED_LINK}"
    # Disable default if present
    rm -f /etc/nginx/sites-enabled/default
fi

# 7. Check if SSL certificate exists, or obtain via Certbot
echo "[+] Step 7: Configuring SSL Certificate..."
if [[ ! -d "/etc/letsencrypt/live/${DOMAIN}" ]]; then
    echo "[*] Requesting Let's Encrypt certificate for ${DOMAIN}..."
    # Temporary bootstrap: comment out SSL directives to allow certbot to run HTTP-01 challenge
    sed -i.bak 's/listen 443 ssl http2;/listen 443; # temporary/g' "${NGINX_CONF_TARGET}"
    sed -i 's/ssl_certificate/#ssl_certificate/g' "${NGINX_CONF_TARGET}"
    
    nginx -t && systemctl reload nginx || systemctl restart nginx
    
    certbot certonly --nginx --non-interactive --agree-tos --email "${EMAIL}" -d "${DOMAIN}" || true

    # Restore SSL config if cert succeeded
    if [[ -d "/etc/letsencrypt/live/${DOMAIN}" ]]; then
        mv "${NGINX_CONF_TARGET}.bak" "${NGINX_CONF_TARGET}"
    else
        echo "[!] Warning: Certbot did not automatically succeed. Ensure DNS for ${DOMAIN} points to this VPS."
    fi
fi

# 8. Test and Reload Nginx
echo "[+] Step 8: Validating Nginx configuration and restarting service..."
nginx -t
systemctl enable nginx
systemctl reload nginx || systemctl restart nginx

# 9. Configure Firewall if UFW active
if command -v ufw &> /dev/null && ufw status | grep -q "Status: active"; then
    echo "[+] Step 9: Opening HTTP (80) and HTTPS (443) ports in UFW firewall..."
    ufw allow 80/tcp
    ufw allow 443/tcp
fi

echo "==================================================================="
echo "  [✓] DEPLOYMENT COMPLETE!"
echo ""
echo "  Portal Landing Page: https://${DOMAIN}/"
echo "  WordCraft Web App:   https://${DOMAIN}/word/"
echo "  Future SheetCraft:   https://${DOMAIN}/sheet/"
echo "  Future SlideCraft:   https://${DOMAIN}/slide/"
echo "==================================================================="
