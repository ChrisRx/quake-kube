// A diagram shrunk to fit the text opens larger when it's clicked, in a dialog over the page
// that scrolls when the window is smaller than the diagram.
"use strict";

{
  const figures = document.querySelectorAll(".content figure.diagram");
  let dialog;

  // The width pikchr drew the diagram at, from mdbook-pikchr.
  const drawn = (fig) => parseFloat(fig.style.getPropertyValue("--diagram-width"));

  // A diagram is worth opening when the text shows it at less than nine tenths of its size.
  const shrunk = (fig) => fig.getBoundingClientRect().width < 0.9 * drawn(fig);

  const mark = () => {
    for (const fig of figures) {
      const on = shrunk(fig);
      fig.classList.toggle("zoomable", on);
      if (on) {
        fig.setAttribute("role", "button");
        fig.setAttribute("tabindex", "0");
        fig.setAttribute("aria-label", "Open the diagram larger");
        fig.title = "Click to enlarge";
      } else {
        fig.removeAttribute("role");
        fig.removeAttribute("tabindex");
        fig.removeAttribute("aria-label");
        fig.removeAttribute("title");
      }
    }
  };

  const open = (fig) => {
    if (!dialog) {
      dialog = document.createElement("dialog");
      dialog.className = "diagram-zoom";
      dialog.innerHTML = `<button class="close" type="button" aria-label="Close">×</button><div class="scroll"></div>`;
      dialog.querySelector(".close").addEventListener("click", () => dialog.close());
      // A click outside the diagram closes it too.
      dialog.addEventListener("click", (e) => {
        if (!e.target.closest("svg")) dialog.close();
      });
      document.body.append(dialog);
    }
    const svg = fig.querySelector("svg").cloneNode(true);
    // Fit to the window, up to its drawn size, where its text is the page's size. A small
    // window scrolls it rather than shrinking its text further.
    const width = Math.min(drawn(fig), Math.max(innerWidth - 64, 0.6 * drawn(fig)));
    svg.style.width = `${width}px`;
    dialog.querySelector(".scroll").replaceChildren(svg);
    dialog.showModal();
  };

  for (const fig of figures) {
    fig.addEventListener("click", () => {
      if (fig.classList.contains("zoomable")) open(fig);
    });
    fig.addEventListener("keydown", (e) => {
      if ((e.key === "Enter" || e.key === " ") && fig.classList.contains("zoomable")) {
        e.preventDefault();
        open(fig);
      }
    });
  }
  if (figures.length) {
    mark();
    addEventListener("resize", mark);
    // mdBook's keyboard shortcuts cancel Esc, which would otherwise close the dialog.
    addEventListener(
      "keydown",
      (e) => {
        if (e.key === "Escape" && dialog?.open) {
          e.stopImmediatePropagation();
          dialog.close();
        }
      },
      true,
    );
  }
}
