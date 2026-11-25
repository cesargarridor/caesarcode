package com.example

sealed class Result {
    data class Ok(val v: Int) : Result()
}
