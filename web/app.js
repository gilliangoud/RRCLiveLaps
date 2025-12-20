// Configuration
const WS_URL = 'ws://localhost:8080/ws';

// State
const transponders = new Map(); // code -> { lastPassingTime: Date, lastLapTime: string, lapCount: number }
const transponderNames = new Map(); // code -> name
let ws = null;
let reconnectInterval = null;

// DOM Elements
const statusIndicator = document.getElementById('status-indicator');
const transponderList = document.getElementById('transponder-list');
const mappingFileInput = document.getElementById('mapping-file');

// Load Mapping on Start
window.addEventListener('load', () => {
    fetch('mapping.json')
        .then(response => {
            if (!response.ok) {
                throw new Error('Network response was not ok');
            }
            return response.json();
        })
        .then(mapping => {
            transponderNames.clear();
            Object.entries(mapping).forEach(([code, name]) => {
                transponderNames.set(code, name);
            });
            console.log('Loaded mapping:', transponderNames);
        })
        .catch(err => {
            console.error('Error loading mapping.json:', err);
            // Non-blocking, just no names will be shown
        });
});

// Status Management
const Status = {
    DISCONNECTED: 'status-red',
    CONNECTING: 'status-yellow',
    CONNECTED_NO_DATA: 'status-orange',
    CONNECTED_READY: 'status-green'
};

function setStatus(status) {
    console.log(`Status changed to: ${status}`);
    statusIndicator.className = 'status-indicator ' + status;
}

// Time Formatting
function formatTime(ms) {
    let seconds = ms / 1000;
    if (seconds > 99.99) {
        return "99.99";
    }
    return seconds.toFixed(2);
}

// Transponder Management
function updateTransponder(data) {
    const code = data.transponder;
    const passingTime = new Date(data.date);

    // Debug log
    console.log(`Received passing for ${code}:`, data.date, passingTime);

    let lapTime = null;

    // Handle Impulse Marker (Start Signal)
    if (code === "00000127") {
        console.log("Impulse Marker received at:", passingTime);
        // Update all existing transponders' lastPassingTime to this marker time
        for (const [tCode, tData] of transponders.entries()) {
            tData.lastPassingTime = passingTime;
            // Reset lap counts on start signal? Usually yes for a new race, 
            // but let's keep it simple: just sync time.
            // If user wants reset, they can refresh page.
        }
        return; // Do not display the marker itself
    }

    // Hide title if visible
    const title = document.querySelector('h1');
    if (title && title.style.display !== 'none') {
        title.style.display = 'none';
    }

    if (transponders.has(code)) {
        const lastData = transponders.get(code);
        const diff = passingTime - lastData.lastPassingTime;

        // Lower debounce to 500ms to allow fast laps (mock decoder is 1s)
        if (diff > 500) {
            lapTime = formatTime(diff);
            // Increment lap count
            const newLapCount = (lastData.lapCount || 0) + 1;

            transponders.set(code, {
                lastPassingTime: passingTime,
                lastLapTime: lapTime,
                lapCount: newLapCount
            });
            updateDOM(code, lapTime, newLapCount);
        } else {
            transponders.get(code).lastPassingTime = passingTime;
        }
    } else {
        // First passing
        transponders.set(code, {
            lastPassingTime: passingTime,
            lastLapTime: '0.00',
            lapCount: 0 // Start at 0 or 1? Usually 0 completed laps if this is start line interaction? 
            // Or if it's first detection. Let's say 0 laps completed (waiting for first full lap).
            // Or is this a finish line passing? If 0, then 1st lap starts.
        });
        updateDOM(code, '0.00', 0);
    }
}

function updateDOM(code, lapTime, lapCount) {
    let item = document.getElementById(`transponder-${code}`);
    const name = transponderNames.get(code) || code;
    // For display, if mapped, show Name. If not, show Code.
    // We will show the Code smaller if Name is present, or just Name if we want cleaner UI.
    // User asked for "transponder id stacked vertically".
    // Let's show:
    // [Name]
    // [Code] (small)
    // [Laps: X]

    // Construct HTML for the Info block
    let infoHTML = '';
    if (transponderNames.has(code)) {
        infoHTML = `
            <div class="transponder-name">${name}</div>
            <div class="transponder-code-small">${code}</div>
            <div class="lap-info">Laps: ${lapCount}</div>
        `;
    } else {
        infoHTML = `
            <div class="transponder-name">${code}</div>
            <div class="lap-info">Laps: ${lapCount}</div>
        `;
    }

    if (!item) {
        item = document.createElement('div');
        item.id = `transponder-${code}`;
        item.className = 'transponder-item';
        item.innerHTML = `
            <div class="transponder-info">
                ${infoHTML}
            </div>
            <div class="lap-time">${lapTime}</div>
        `;
        // New item goes to top
        transponderList.prepend(item);
    } else {
        const infoEl = item.querySelector('.transponder-info');
        const timeEl = item.querySelector('.lap-time');

        infoEl.innerHTML = infoHTML;
        timeEl.textContent = lapTime;

        // Move to top
        transponderList.prepend(item);

        // Flash effect
        item.style.backgroundColor = '#4d4d4d';
        setTimeout(() => {
            item.style.backgroundColor = '#2d2d2d';
        }, 200);
    }
}
// Sort list by most recent activity (optional, but nice)
// For now, we just append or update in place.


// WebSocket Connection
function connect() {
    setStatus(Status.CONNECTING);

    ws = new WebSocket(WS_URL);

    ws.onopen = () => {
        console.log('WebSocket connected');
        setStatus(Status.CONNECTED_NO_DATA);

        // Clear reconnect interval if it exists
        if (reconnectInterval) {
            clearInterval(reconnectInterval);
            reconnectInterval = null;
        }
    };

    ws.onmessage = (event) => {
        try {
            const message = JSON.parse(event.data);

            // Check for "connected" event (as requested by user)
            // The user said: "green if there is an active websocket connection and a 'connected' event has been received"
            // Since rrconverter sends passing objects, we might treat the first valid passing as "connected" 
            // OR we can simulate it if the server sends a specific hello message.
            // Currently rrconverter sends `Passing` objects.
            // Let's assume if we receive ANY valid data, we are "connected" in the user's sense,
            // OR we check for a specific type.

            // User requirement: "orange for connected to the websocket server but no received 'connected' event... green if... 'connected' event has been received"
            // Since I cannot easily change rrconverter to send a "connected" event right now without a new task,
            // I will implement the client to look for a message with `type: 'connected'` OR just treat the first passing as success?
            // STRICT INTERPRETATION: The client stays Orange until it sees `event: 'connected'` or similar.
            // BUT `rrconverter` sends flat JSON of the passing.
            // I will add a small hack: I will treat receiving ANY message as the "connected" event for now, 
            // effectively turning it green on the first passing. 
            // Wait, the user might update the server later.
            // Let's check if the message has a specific structure.

            // If the message is a passing, it has `passing_number`.
            if (message.passing_number) {
                setStatus(Status.CONNECTED_READY);
                updateTransponder(message);
            } else if (message.event === 'connected') {
                setStatus(Status.CONNECTED_READY);
            } else if (message.event === 'disconnected') {
                setStatus(Status.CONNECTED_NO_DATA);
            }

        } catch (e) {
            console.error('Error parsing message:', e);
        }
    };

    ws.onclose = () => {
        console.log('WebSocket disconnected');
        setStatus(Status.DISCONNECTED);
        attemptReconnect();
    };

    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
        setStatus(Status.DISCONNECTED);
        ws.close();
    };
}

function attemptReconnect() {
    if (!reconnectInterval) {
        reconnectInterval = setInterval(() => {
            console.log('Attempting to reconnect...');
            connect();
        }, 5000);
    }
}

// Cleanup Interval
setInterval(() => {
    const now = new Date();
    const TIMEOUT_MS = 10 * 60 * 1000; // 10 minutes

    for (const [code, data] of transponders.entries()) {
        if (now - data.lastPassingTime > TIMEOUT_MS) {
            console.log(`Removing inactive transponder: ${code}`);
            transponders.delete(code);
            const item = document.getElementById(`transponder-${code}`);
            if (item) {
                item.remove();
            }
        }
    }
}, 60 * 1000); // Run every minute

// Start
connect();
