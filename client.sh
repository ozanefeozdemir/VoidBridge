#!/bin/bash

# --- AYARLAR ---
GITHUB_USER="ozanefeozdemir"
REPO_NAME="VoidBridge"
BINARY_NAME="voidbridge-linux-amd64"
# ---------------

# 1. Sunucu IP Adresini Al (Otomatik Kontrol)
# Kullanıcı komutu "./client.sh 10.0.2.16" diye yazdıysa IP'yi oradan al
SERVER_IP=$1

# Yazmadıysa, ekrana sor
if [ -z "$SERVER_IP" ]; then
    echo "------------------------------------------------"
    echo "VoidBridge İstemcisine Hoş Geldiniz!"
    echo "Lütfen bağlanmak istediğiniz Sunucu IP adresini girin:"
    read -p "IP Adresi: " SERVER_IP
fi

# Hala boşsa çıkış yap
if [ -z "$SERVER_IP" ]; then
    echo "HATA: IP adresi girilmedi. İşlem iptal edildi."
    exit 1
fi

# 2. İndirme Linkini Dinamik Olarak Bul (Latest Release)
DOWNLOAD_URL="https://github.com/$GITHUB_USER/$REPO_NAME/releases/latest/download/$BINARY_NAME"

echo ">>> VoidBridge Hazırlanıyor..."

# 3. Dosya yoksa indir
if [ ! -f "./$BINARY_NAME" ]; then
    echo ">>> İstemci GitHub'dan indiriliyor..."
    wget -q --show-progress -O "$BINARY_NAME" "$DOWNLOAD_URL"
    
    if [ $? -ne 0 ]; then
        echo "HATA: İndirme başarısız! İnternet bağlantınızı kontrol edin."
        exit 1
    fi
    
    chmod +x "$BINARY_NAME"
    echo ">>> İndirme Tamam."
fi

# 4. Bağlan
echo ">>> $SERVER_IP sunucusuna bağlanılıyor..."
sudo ./$BINARY_NAME client --remote-ip $SERVER_IP --port 9000