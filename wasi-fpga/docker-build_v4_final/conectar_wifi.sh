#!/bin/bash

# 1. Verificación de privilegios
if [ "$EUID" -ne 0 ]; then 
  echo "Error: Debes ejecutar este script con sudo."
  exit 1
fi

SSID="$1"
PASS="$2"
IP_CIDR="$3"

if [ -z "$SSID" ] || [ -z "$PASS" ]; then
    echo "Uso: sudo $0 'SSID' 'Password' ['IP/Mask']"
    exit 1
fi

echo "=========================================="
echo "    GESTOR WIFI + INSTALADOR OFFLINE"
echo "=========================================="

# 2. Gestión del Driver (Offline/Online)
if ! lsmod | grep -q "8188eu"; then
    echo "[!] Driver 8188eu no detectado."
    
    # Intentar ver si los archivos ya están aquí (descargados previamente)
    if [ -d "./rtl8188eu" ]; then
        echo "[0/4] Carpeta de driver detectada localmente. Instalando..."
        cd rtl8188eu
        make && make install
        modprobe -r r8188eu 2>/dev/null
        depmod -a && modprobe 8188eu
        cd ..
    else
        # Si no están, ver si hay internet para descargarlos
        echo "Buscando archivos en internet..."
        if ping -c 1 google.com > /dev/null 2>&1; then
            echo "[0/4] Internet detectado. Descargando driver..."
            apt update && apt install -y build-essential linux-headers-$(uname -r) git
            git clone https://github.com/lwfinger/rtl8188eu.git
            cd rtl8188eu && make && make install
            modprobe -r r8188eu 2>/dev/null
            depmod -a && modprobe 8188eu
            cd ..
        else
            echo "-------------------------------------------------------"
            echo "ERROR: No hay internet ni carpeta local './rtl8188eu'."
            echo "INSTRUCCIONES PARA MODO OFFLINE:"
            echo "1. Desde un portátil con internet, descarga el ZIP de:"
            echo "   https://github.com/lwfinger/rtl8188eu/archive/refs/heads/master.zip"
            echo "2. Descomprímelo en tu SD o pendrive."
            echo "3. Asegúrate de que la carpeta se llame 'rtl8188eu' y esté"
            echo "   en el mismo sitio que este script."
            echo "-------------------------------------------------------"
            exit 1
        fi
    fi
    echo "OK: Driver cargado."
    sleep 2
else
    echo "[0/4] Driver ya cargado. Continuando..."
fi

# 3. Despertar WiFi
nmcli radio wifi on
rfkill unblock wifi > /dev/null 2>&1
sleep 1

# 4. Escaneo con reintentos
echo "[1/4] Escaneando redes..."
MAX_RETRIES=6
COUNT=0
while [ $COUNT -lt $MAX_RETRIES ]; do
    RED_EXISTE=$(nmcli -t -f SSID device wifi list | grep -w "^$SSID$")
    if [ -n "$RED_EXISTE" ]; then break; fi
    echo "Intento $((COUNT+1)): Buscando '$SSID'..."
    nmcli device wifi rescan 2>/dev/null
    sleep 3
    COUNT=$((COUNT+1))
done

if [ -z "$RED_EXISTE" ]; then
    echo "ERROR: La red '$SSID' no aparece. ¿Está encendido el Router/Honor?"
    exit 1
fi

# 5. Conexión
echo "[2/4] Conectando..."
nmcli connection delete "$SSID" > /dev/null 2>&1
if [ -z "$IP_CIDR" ]; then
    nmcli device wifi connect "$SSID" password "$PASS" name "$SSID"
else
    GW=$(echo $IP_CIDR | cut -d. -f1-3).1
    nmcli device wifi connect "$SSID" password "$PASS" name "$SSID" \
          ipv4.method manual ipv4.addresses "$IP_CIDR" ipv4.gateway "$GW" ipv4.dns "8.8.8.8"
fi

# 6. Finalización
echo "[3/4] Verificando..."
sleep 2
MI_IP=$(nmcli -g ip4.address connection show "$SSID")
echo "IP: $MI_IP"

if ping -c 2 8.8.8.8 > /dev/null 2>&1; then
    echo "=========================================="
    echo "   SISTEMA CONECTADO Y ONLINE"
    echo "=========================================="
else
    echo "AVISO: Conexión local establecida, pero sin salida a Internet."
fi