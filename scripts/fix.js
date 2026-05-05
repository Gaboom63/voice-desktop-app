// This runs inside Google Voice
console.log("Modernizer script injected!");

// Logic to find unread messages without relying on fragile CSS classes
function checkUnread() {
    // Google Voice usually puts unread count in the document title: "Google Voice (3)"
    const title = document.title;
    const match = title.match(/\((\d+)\)/);
    const count = match ? parseInt(match[1]) : 0;

    // Use Tauri's internal event system to tell the taskbar
    if (window.__TAURI__) {
        window.__TAURI__.event.emit('unread-update', { count });
    }
}

// Check every 3 seconds
setInterval(checkUnread, 3000);
