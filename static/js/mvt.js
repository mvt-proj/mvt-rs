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

let OPEN_DROPDOWN_ID = null;

function toggleDropdown(id) {
  const menu = document.getElementById(`dropdown-menu-${id}`);
  if (!menu) return;
  const isOpen = !menu.classList.contains('hidden');

  closeAllDropdowns();

  if (!isOpen) openDropdown(id);
}

function openDropdown(id) {
  const menu = document.getElementById(`dropdown-menu-${id}`);
  const btn = document.querySelector(`[data-dropdown="${id}"] button`);
  if (!menu || !btn) return;

  menu.classList.remove('hidden');

  menu.style.position = 'fixed';
  menu.style.maxHeight = '70vh';

  const btnRect = btn.getBoundingClientRect();
  const menuWidth = menu.offsetWidth;
  const menuHeight = menu.offsetHeight;

  const gapY = -5;
  const gapX = 30;

  const preferredLeft = btnRect.left + (btnRect.width / 2) - (menuWidth / 2);
  const minLeft = gapX;
  const maxLeft = window.innerWidth - menuWidth - gapX;
  const left = Math.min(Math.max(preferredLeft, minLeft), maxLeft);

  const spaceBelow = window.innerHeight - btnRect.bottom;
  const spaceAbove = btnRect.top;

  let top;
  if (spaceBelow < menuHeight + gapY && spaceAbove > menuHeight + gapY) {
    top = btnRect.top - menuHeight - gapY - 16;
  } else {
    top = btnRect.bottom + gapY;
  }

  menu.style.left = `${Math.round(left)}px`;
  menu.style.top = `${Math.round(top)}px`;

  menu.style.right = '';
  menu.style.zIndex = '2000';
  menu.style.width = `${menuWidth}px`;

  OPEN_DROPDOWN_ID = id;
}

function closeDropdown(id) {
  const menu = document.getElementById(`dropdown-menu-${id}`);
  if (menu) {
    menu.classList.add('hidden');
    menu.style.position = '';
    menu.style.top = '';
    menu.style.right = '';
    menu.style.width = '';
    menu.style.maxHeight = '';
    menu.style.zIndex = '';
  }
  if (OPEN_DROPDOWN_ID === id) OPEN_DROPDOWN_ID = null;
}

function closeAllDropdowns() {
  document.querySelectorAll('[id^="dropdown-menu-"]').forEach(m => {
    m.classList.add('hidden');
    m.style.position = '';
    m.style.top = '';
    m.style.right = '';
    m.style.width = '';
    m.style.maxHeight = '';
    m.style.zIndex = '';
  });
  OPEN_DROPDOWN_ID = null;
}

document.addEventListener('click', (e) => {
  if (!e.target.closest('[data-dropdown]')) closeAllDropdowns();
});

window.addEventListener('resize', () => {
  if (OPEN_DROPDOWN_ID) {
    const id = OPEN_DROPDOWN_ID;
    closeDropdown(id);
    openDropdown(id);
  }
});

window.addEventListener('scroll', () => {
  if (OPEN_DROPDOWN_ID) {
    const id = OPEN_DROPDOWN_ID;
    const menu = document.getElementById(`dropdown-menu-${id}`);
    const btn = document.querySelector(`[data-dropdown="${id}"] button`);
    if (menu && btn && !menu.classList.contains('hidden')) openDropdown(id);
  }
}, { passive: true });

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeAllDropdowns();
});
