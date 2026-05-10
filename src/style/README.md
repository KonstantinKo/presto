# CSS Architecture Documentation

Questo documento descrive la nuova architettura CSS modulare per l'applicazione Pomodoro Timer.

## Struttura dei File

### 📁 `styles/`

La cartella `styles` contiene tutti i file CSS organizzati per funzionalità:

#### **Core Styles**
- **`variables.css`** - Variabili CSS, colori, font e stili base
- **`timer.css`** - Stili per il display del timer principale
- **`controls.css`** - Pulsanti di controllo e loro stati
- **`progress.css`** - Indicatori di progresso e punti pomodoro

#### **UI Components**
- **`tasks.css`** - Sezione gestione task
- **`statistics.css`** - Sezione statistiche
- **`modals.css`** - Modal dialogs e finestre popup
- **`notifications.css`** - Sistema di notifiche

#### **Layout & Navigation**
- **`layout.css`** - Layout generale e container delle viste
- **`sidebar.css`** - Barra laterale di navigazione

#### **Views & Pages**
- **`calendar.css`** - Vista calendario con grafici e timeline
- **`settings.css`** - Pagina impostazioni e configurazioni
- **`team.css`** - Vista team per collaborazione
- **`timeline.css`** - Timeline sessioni e gestione sessioni

#### **Effects & Responsive**
- **`animations.css`** - Animazioni e transizioni
- **`responsive.css`** - Design responsivo e stili mobile

#### **Entry Point**
- **`main.css`** - File principale che importa tutti i moduli

## Vantaggi della Nuova Architettura

### 🚀 **Manutenibilità**
- Ogni file si occupa di una specifica funzionalità
- Facile individuare e modificare stili specifici
- Riduzione dei conflitti CSS

### 🎯 **Organizzazione**
- Separazione logica delle responsabilità
- Struttura prevedibile e intuitiva
- Documentazione implicita tramite nomi dei file

### 📱 **Performance**
- CSS caricato in modo modulare
- Possibilità di ottimizzazioni future (lazy loading)
- Bundle size ottimizzato

### 🔧 **Sviluppo**
- Lavoro parallelo su componenti diversi
- Conflitti Git ridotti
- Debug più semplice

## Come Utilizzare

### Import nel HTML
```html
<link rel="stylesheet" href="styles/main.css" />
```

### Modificare Stili Specifici
- **Timer**: modifica `timer.css`
- **Colori**: modifica `variables.css`  
- **Mobile**: modifica `responsive.css`
- **Calendario**: modifica `calendar.css`

### Aggiungere Nuovi Stili
1. Identifica la categoria appropriata
2. Modifica il file CSS corrispondente
3. Se necessario, crea un nuovo file e aggiornalo in `main.css`

## Struttura delle Variabili CSS

Le variabili principali sono definite in `variables.css`:

```css
:root {
  /* Timer Colors */
  --focus-color: #e74c3c;
  --break-color: #2ecc71;
  --long-break-color: #3498db;
  --accent-color: #9b59b6;
  
  /* Background Colors */
  --focus-bg: #FFF2F2;
  --break-bg: #F0FAF0;
  --long-break-bg: #E8F4FF;
  
  /* Button Colors */
  --focus-primary-btn: #FF7c7c;
  --break-primary-btn: #8CE8A1;
  --long-break-primary-btn: #8BCAFF;
}
```

## Best Practices

### 📝 **Convenzioni di Naming**
- Usa nomi descrittivi per le classi
- Prefissi per componenti specifici (es. `calendar-`, `timer-`)
- Variabili CSS per valori riutilizzati

### 🎨 **Organizzazione del Codice**
- Commenti per sezioni logiche
- Raggruppamento di proprietà correlate
- Uso consistente di indentazione

### 📱 **Responsività**
- Mobile-first approach
- Breakpoint consistenti
- Progressive enhancement

## Migrazione dal Vecchio Sistema

Il vecchio file `styles.css` è stato suddiviso mantenendo:
- ✅ Tutti gli stili esistenti
- ✅ Funzionalità invariate  
- ✅ Compatibilità completa
- ✅ Performance migliorate

## File di Backup

Il file originale `styles.css` rimane disponibile come backup e riferimento.
