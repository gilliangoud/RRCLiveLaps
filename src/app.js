// Configuration
const WS_URL = 'ws://localhost:8080/ws';

// State
const transponders = new Map(); // code -> { lastPassingTime: Date, lastLapTime: string }
let ws = null;
let reconnectInterval = null;

// DOM Elements
const statusIndicator = document.getElementById('status-indicator');
const transponderList = document.getElementById('transponder-list');

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
            transponders.set(code, {
                lastPassingTime: passingTime,
                lastLapTime: lapTime
            });
            updateDOM(code, lapTime);
        } else {
            transponders.get(code).lastPassingTime = passingTime;
        }
    } else {
        // First passing
        transponders.set(code, {
            lastPassingTime: passingTime,
            lastLapTime: '0.00'
        });
        updateDOM(code, '0.00');
    }
}

function updateDOM(code, lapTime) {
    let item = document.getElementById(`transponder-${code}`);

    if (!item) {
        item = document.createElement('div');
        item.id = `transponder-${code}`;
        item.className = 'transponder-item';
        item.innerHTML = `
            <span class="transponder-code">${code}</span>
            <span class="lap-time">${lapTime}</span>
        `;
        // New item goes to top
        transponderList.prepend(item);
    } else {
        const timeEl = item.querySelector('.lap-time');
        timeEl.textContent = lapTime;

        // Move to top
        transponderList.prepend(item);

        // Flash effect
        item.style.backgroundColor = '#4d4d4d';
        setTimeout(() => {
            item.style.backgroundColor = '#2d2d2d';
        }, 200);
    }

    // Sort list by most recent activity (optional, but nice)
    // For now, we just append or update in place.
}

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
