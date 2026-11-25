package com.example.model;

import jakarta.persistence.Entity;

@Entity
@Table(name = "users")
public class User {
    private Long id;
}
