package encrypter

import (
	"crypto/cipher"
	"crypto/rand"
	"errors"
	"io"
)

// aesgcm is an Encrypter implementation that uses AES-GCM (Advanced Encryption
// Standard in Galois/Counter Mode) for authenticated encryption. AES-GCM provides
// both confidentiality and authenticity, protecting against tampering.
type aesgcm struct {
	block cipher.Block
}

// Encrypt encrypts the plaintext string using AES-GCM with a randomly generated nonce.
// The nonce is prepended to the ciphertext, so Decrypt can extract it during decryption.
//
// Returns the encrypted data as a byte slice containing: [nonce || ciphertext || auth_tag],
// or an error if encryption fails.
func (e *aesgcm) Encrypt(plaintext string) ([]byte, error) {
	gcm, err := cipher.NewGCM(e.block)
	if err != nil {
		return nil, err
	}

	nonce := make([]byte, gcm.NonceSize())
	_, err = io.ReadFull(rand.Reader, nonce)
	if err != nil {
		return nil, err
	}

	return gcm.Seal(nonce, nonce, []byte(plaintext), nil), nil
}

// Decrypt decrypts the ciphertext using AES-GCM, extracting the nonce from
// the beginning of the ciphertext as it was prepended during encryption.
//
// Returns the decrypted plaintext as a string, or an error if decryption fails
// (e.g., due to malformed ciphertext or authentication failure).
func (e *aesgcm) Decrypt(ciphertext []byte) (string, error) {
	gcm, err := cipher.NewGCM(e.block)
	if err != nil {
		return "", err
	}

	if len(ciphertext) < gcm.NonceSize() {
		return "", errors.New("malformed ciphertext")
	}

	plaintext, err := gcm.Open(nil,
		ciphertext[:gcm.NonceSize()],
		ciphertext[gcm.NonceSize():],
		nil,
	)

	return string(plaintext), err
}
