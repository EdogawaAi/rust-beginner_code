// Simple Hangman Program
// User gets five incorrect guesses
// Word chosen randomly from words.txt
// Inspiration from: https://doc.rust-lang.org/book/ch02-00-guessing-game-tutorial.html
// This assignment will introduce you to some fundamental syntax in Rust:
// - variable declaration
// - string manipulation
// - conditional statements
// - loops
// - vectors
// - files
// - user input
// We've tried to limit/hide Rust's quirks since we'll discuss those details
// more in depth in the coming lectures.
extern crate rand;
use rand::Rng;
use std::fs;
use std::io;
use std::io::Write;

const NUM_INCORRECT_GUESSES: u32 = 5;
const WORDS_PATH: &str = "words.txt";

fn pick_a_random_word() -> String {
    let file_string = fs::read_to_string(WORDS_PATH).expect("Unable to read file.");
    let words: Vec<&str> = file_string.split('\n').collect();
    String::from(words[rand::thread_rng().gen_range(0, words.len())].trim())
}

fn main() {
    let secret_word = pick_a_random_word();
    // Note: given what you know about Rust so far, it's easier to pull characters out of a
    // vector than it is to pull them out of a string. You can get the ith character of
    // secret_word by doing secret_word_chars[i].
    let secret_word_chars: Vec<char> = secret_word.chars().collect();
    // Uncomment for debugging:
    // println!("random word: {}", secret_word);

    // Your code here! :)
    let mut so_far : String = "-".repeat(secret_word.len());
    let mut guessed_letters : String = String::new();
    let mut remaining_guess = NUM_INCORRECT_GUESSES;

    println!("Welcome to CS110L Hangman!");

    while remaining_guess > 0 && so_far.contains('-') {
        println!("The word so far is {}", so_far);
        println!("You have guessed the following letters: {}", guessed_letters);
        println!("You have {} guesses left", remaining_guess);

        print!("Please guess a letter: ");
        // Make sure the prompt from the previous line gets displayed:
        io::stdout()
            .flush()
            .expect("Error flushing stdout.");
        let mut guess = String::new();
        io::stdin()
            .read_line(&mut guess)
            .expect("Error reading line.");
        let guess_char = guess.trim().chars().next().expect("Please enter a valid character.");
        println!("{}", guess_char);
        println!("");

        if guessed_letters.contains(guess_char){
            println!("You've guessed that char");
            continue;
        }

        guessed_letters.push(guess_char);

        if !secret_word_chars.contains(&guess_char){
            println!("Sorry, that letter is not in the word");
            remaining_guess -= 1;
        }
        else {
            for (i, &c) in secret_word_chars.iter().enumerate() {
                if secret_word_chars[i] == guess_char {
                    so_far.replace_range(i..= i, &guess_char.to_string());
                }
            }
        }

        println!("");
    }

    if so_far.contains('-'){
        println!("Sorry, you ran out of guesses!");
    }
    else{
        println!("Congratulations you guessed the secret word: {}!", secret_word);
    }

}
