// Izhaan Intellect Office Portal Interactive Scripts

document.addEventListener('DOMContentLoaded', () => {
  // Desktop Downloads Modal Handling
  const modal = document.getElementById('downloads-modal');
  const openButtons = document.querySelectorAll('[data-open-modal="downloads"]');
  const closeButton = document.getElementById('close-modal');

  function openModal() {
    modal.classList.add('active');
    document.body.style.overflow = 'hidden';
  }

  function closeModal() {
    modal.classList.remove('active');
    document.body.style.overflow = '';
  }

  openButtons.forEach(btn => {
    btn.addEventListener('click', (e) => {
      e.preventDefault();
      openModal();
    });
  });

  if (closeButton) {
    closeButton.addEventListener('click', closeModal);
  }

  window.addEventListener('click', (e) => {
    if (e.target === modal) {
      closeModal();
    }
  });

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && modal.classList.contains('active')) {
      closeModal();
    }
  });

  // Drag and Drop Handling for DOCX/DOC/ODT files
  const dropzone = document.getElementById('file-dropzone');
  const fileInput = document.getElementById('file-input');

  if (dropzone && fileInput) {
    ['dragenter', 'dragover'].forEach(eventName => {
      dropzone.addEventListener(eventName, (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.add('drag-over');
      });
    });

    ['dragleave', 'drop'].forEach(eventName => {
      dropzone.addEventListener(eventName, (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.remove('drag-over');
      });
    });

    dropzone.addEventListener('click', () => {
      fileInput.click();
    });

    dropzone.addEventListener('drop', (e) => {
      const files = e.dataTransfer.files;
      if (files && files.length > 0) {
        handleFileSelection(files[0]);
      }
    });

    fileInput.addEventListener('change', (e) => {
      if (fileInput.files && fileInput.files.length > 0) {
        handleFileSelection(fileInput.files[0]);
      }
    });
  }

  function handleFileSelection(file) {
    const validExtensions = ['.docx', '.doc', '.odt', '.rtf', '.txt', '.md'];
    const fileName = file.name.toLowerCase();
    const isValid = validExtensions.some(ext => fileName.endsWith(ext));

    if (!isValid) {
      alert('Please upload a valid document file (.docx, .doc, .odt, .rtf, .txt, .md)');
      return;
    }

    const titleEl = document.querySelector('.dropzone-title');
    const subEl = document.querySelector('.dropzone-sub');
    if (titleEl) titleEl.textContent = `Opening ${file.name}...`;
    if (subEl) subEl.textContent = 'Transferring file into WordCraft engine...';

    // Read the file as ArrayBuffer and store temporarily in sessionStorage
    const reader = new FileReader();
    reader.onload = function(evt) {
      try {
        const bytes = new Uint8Array(evt.target.result);
        // Save in sessionStorage or forward directly
        sessionStorage.setItem('pending_doc_name', file.name);
        // Redirect directly to the web app
        window.location.href = '/word/';
      } catch (err) {
        console.error('File load error:', err);
        window.location.href = '/word/';
      }
    };
    reader.readAsArrayBuffer(file);
  }

  // Smooth scroll for nav anchor links
  document.querySelectorAll('a[href^="#"]').forEach(anchor => {
    anchor.addEventListener('click', function (e) {
      const targetId = this.getAttribute('href');
      if (targetId === '#' || targetId === '') return;
      const targetEl = document.querySelector(targetId);
      if (targetEl) {
        e.preventDefault();
        targetEl.scrollIntoView({
          behavior: 'smooth'
        });
      }
    });
  });
});
