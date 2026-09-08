module.exports = {
  content: ["./src/**/*.rs", "./index.html"],
  theme: {
    extend: {
      colors: {
        paper: "#ffffff",
        ink: "#000000",
        inksoft: "#737373",
      },
      fontFamily: {
        serif: ['"Noto Serif TC"', "serif"],
        sans: ['"Noto Sans TC"', "sans-serif"],
        mono: ["ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
