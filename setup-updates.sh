#!/bin/bash

# Script di configurazione per Tempo Update System
# Questo script ti guiderà attraverso la configurazione degli aggiornamenti automatici

echo "🍅 Tempo - Configurazione Sistema Aggiornamenti"
echo "=================================================="
echo ""

# Funzione per richiedere input
read_input() {
    local prompt="$1"
    local variable_name="$2"
    local default_value="$3"
    
    if [ -n "$default_value" ]; then
        read -p "$prompt [$default_value]: " input
        if [ -z "$input" ]; then
            input="$default_value"
        fi
    else
        read -p "$prompt: " input
    fi
    
    eval "$variable_name='$input'"
}

# Raccolta informazioni
echo "1. Configurazione Repository GitHub"
echo "-----------------------------------"
read_input "Username GitHub" github_username
read_input "Nome Repository" github_repo "tempo"

echo ""
echo "2. Configurazione Chiavi"
echo "------------------------"
read_input "Nome file chiave" key_name "tempo_signing_key"

# Creazione della directory per le chiavi
key_dir="$HOME/.tauri"
mkdir -p "$key_dir"

echo ""
echo "📝 Generazione chiavi di firma..."

# Controlla se tauri CLI è disponibile
if ! command -v tauri &> /dev/null; then
    echo "❌ Tauri CLI non trovato. Installandolo..."
    npm install --save-dev @tauri-apps/cli@latest
    
    if ! command -v npx &> /dev/null; then
        echo "❌ NPM non trovato. Installa Node.js prima di continuare."
        exit 1
    fi
    
    # Usa npx se tauri non è nel PATH
    TAURI_CMD="npx tauri"
else
    TAURI_CMD="tauri"
fi

# Genera le chiavi
echo "🔑 Generazione keypair..."
$TAURI_CMD signer generate -w "$key_dir/$key_name"

if [ $? -eq 0 ]; then
    echo "✅ Chiavi generate con successo!"
    
    # Ottieni la chiave pubblica
    echo ""
    echo "🔑 La tua chiave pubblica è:"
    echo "----------------------------------------"
    public_key=$($TAURI_CMD signer sign -k "$key_dir/$key_name" --password "" 2>/dev/null | head -1)
    echo "$public_key"
    echo "----------------------------------------"
    
    # Aggiorna tauri.conf.json
    echo ""
    echo "📝 Aggiornamento configurazione..."
    
    # Sostituisce i placeholder nel file di configurazione
    config_file="src-tauri/tauri.conf.json"
    if [ -f "$config_file" ]; then
        # Backup del file originale
        cp "$config_file" "$config_file.backup"
        
        # Sostituzioni
        sed -i.tmp "s/{{OWNER}}/$github_username/g" "$config_file"
        sed -i.tmp "s/{{REPO}}/$github_repo/g" "$config_file"
        sed -i.tmp "s/YOUR_PUBLIC_KEY_HERE/$public_key/g" "$config_file"
        rm "$config_file.tmp" 2>/dev/null
        
        echo "✅ Configurazione aggiornata!"
    else
        echo "⚠️  File $config_file non trovato"
    fi
    
    # Aggiorna il main.js con i link del repository
    main_js_file="src/main.js"
    if [ -f "$main_js_file" ]; then
        sed -i.tmp "s/YOUR_USERNAME/$github_username/g" "$main_js_file"
        sed -i.tmp "s/YOUR_REPO/$github_repo/g" "$main_js_file"
        rm "$main_js_file.tmp" 2>/dev/null
        echo "✅ Link repository aggiornati!"
    fi
    
    # Aggiorna l'update manager
    update_manager_file="src/managers/update-manager-global.js"
    if [ -f "$update_manager_file" ]; then
        sed -i.tmp "s/USERNAME\/REPOSITORY/$github_username\/$github_repo/g" "$update_manager_file"
        rm "$update_manager_file.tmp" 2>/dev/null
        echo "✅ Update manager configurato!"
    fi
    
    echo ""
    echo "🎉 Configurazione completata!"
    echo ""
    echo "📋 Prossimi passi:"
    echo "1. Aggiungi questi secrets al tuo repository GitHub:"
    echo "   - TAURI_SIGNING_PRIVATE_KEY: (contenuto di $key_dir/$key_name)"
    echo "   - TAURI_SIGNING_PRIVATE_KEY_PASSWORD: (lascia vuoto se non hai impostato una password)"
    echo ""
    echo "2. Per ottenere la chiave privata:"
    echo "   cat $key_dir/$key_name"
    echo ""
    echo "3. Crea una release su GitHub per testare gli aggiornamenti:"
    echo "   git tag v0.2.0"
    echo "   git push origin v0.2.0"
    echo ""
    echo "4. L'app controllerà automaticamente gli aggiornamenti all'avvio"
    echo ""
    echo "⚠️  IMPORTANTE: Non committare mai la chiave privata nel repository!"
    echo "   La chiave è salvata in: $key_dir/$key_name"
    
else
    echo "❌ Errore nella generazione delle chiavi"
    exit 1
fi
