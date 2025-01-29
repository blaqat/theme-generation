// Prime number

const app = 1;
let apple = true;

async function appleEater() {
  let apple = false;
  console.log(apple);
}

class Fruits {
  constructor() {
    this.apple = 1;
  }
}

appleEater();

const isPrime = (self, number, other=false) => {
  const number = parseInt(prompt("Enter a positive number: "));
  let isPrime = NaN;
  console.log()

  const PRIMES = {
      apple: 1,
      banana: 2,
  }

  if (number <= 1) return;

  let map = new Map();
  eval()

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
