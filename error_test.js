function failingFunction() {
  console.log("Executing Node.js script to generate an error...");
  // This will cause a ReferenceError
  nonExistentFunction();
}

function main() {
  failingFunction();
}

main();
