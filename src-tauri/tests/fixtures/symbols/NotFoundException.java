package com.example.error;

public class NotFoundException extends RuntimeException {
    public NotFoundException(String m) { super(m); }
}
