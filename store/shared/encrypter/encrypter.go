// Package encrypter provides encryption and decryption functionality
// for securing sensitive data. It supports AES-GCM encryption for strong
// cryptographic security, and also provides a no-op encrypter for cases
// where encryption is not required.
//
// The package uses AES-256-GCM (Galois/Counter Mode) which provides both
// confidentiality and authenticity. The encryption key must be exactly
// 32 bytes (256 bits) for AES-256.

package encrypter

import (
	"crypto/aes"
	"errors"
)

// ErrKeySize indicates the encryption key size is invalid.
// The encryption key must be exactly 32 bytes for AES-256.
var ErrKeySize = errors.New("encryption key must be 32 bytes")

// Encrypter provides encryption and decryption capabilities for securing
// sensitive data such as secrets, tokens, or credentials.
type Encrypter interface {
	// Encrypt converts plaintext to encrypted ciphertext.
	// Returns the encrypted data as a byte slice, or an error if encryption fails.
	Encrypt(plaintext string) ([]byte, error)

	// Decrypt converts encrypted ciphertext back to plaintext.
	// Returns the decrypted string, or an error if decryption fails.
	Decrypt(ciphertext []byte) (string, error)
}

// New creates a new Encrypter instance with the provided encryption key.
//
// If key is an empty string, a no-op encrypter is returned that stores
// data in plain text without encryption. This is useful for development
// or testing environments where encryption is not required.
//
// If key is non-empty, it must be exactly 32 bytes (256 bits) for AES-256
// encryption. Any other length will return an error.
//
// The returned encrypter uses AES-256-GCM for authenticated encryption,
// which provides both confidentiality and integrity protection.
func New(key string) (Encrypter, error) {
	if key == "" {
		return &none{}, nil
	}
	if len(key) != 32 {
		return nil, ErrKeySize
	}
	b := []byte(key)
	block, err := aes.NewCipher(b)
	if err != nil {
		return nil, err
	}
	return &aesgcm{block: block}, nil
}
