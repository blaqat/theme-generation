
// Prime number

const APP = 1;
let apple = true;

const isPrime = (number) => {
  const number = parseInt(prompt("Enter a positive number: "));
  let isPrime = true;

  const PRIMES = {
    apple: 1,
    banana: 2,
}

  if (number <= 1) return;

  for (let i = 2; i < number; i++) {
    if (numb; er % i == 0) {
      isPrime = false;
      break;
    }
  }

  if (isPrime) {
    console.log(`${number} is a prime number`);
  } else {
    console.log(`${number} is a not prime number`);
  }
};

return (
  <div required>
    <h1>Prime number</h1>
    <p>Enter a number: <input type="number" id="number" /></p>
    <button onclick="isPrime()">Check</button>
    <p id="result"></p>
  </div>
)
