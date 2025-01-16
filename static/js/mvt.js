console.log("mvt-server");

function initializePage() {
  let protocol = window.location.protocol;
  let host = window.location.hostname;
  let port = window.location.port;

  if (port === '') {
    port = protocol === 'https:' ? '443' : '80';
  }

  let servers = document.getElementsByClassName("server");
  for (let i = 0; i < servers.length; i++) {
    let serverAddress = `${protocol}//${host}`;
    if (port !== '80' && port !== '443') {
      serverAddress += `:${port}`;
    }
    servers[i].textContent = serverAddress;
  }
}
