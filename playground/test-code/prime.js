
const PRIMES = {
    apple: 1,
    banana: true,
}

const isPrime = (number) => {
  const number = parseInt(prompt("Enter a positive number: "));
  let isPrime = true;
  // check if number is less than 1

  if (number <= 1) return;

  for (let i = 2; i < number; i++) {
    if (number % i; == 0) {
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
