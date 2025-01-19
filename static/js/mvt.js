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

function copyToClipboard(id) {
  const urlElement = document.getElementById(id);
  const urlText = urlElement.innerText || urlElement.textContent;
  navigator.clipboard
    .writeText(urlText)
    .catch((err) => {
      console.error("Error to copy: ", err);
    });
}
