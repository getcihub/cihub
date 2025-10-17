package encrypter

// none is an Encrypter implementation that performs no encryption.
// It stores values in plain text, converting between string and []byte formats.
//
// This is useful for development or testing environments where encryption
// is not required, or when the encryption key is intentionally left empty.
// It should NOT be used in production for sensitive data.
type none struct{}

// Encrypt performs no encryption and simply converts the plaintext string to bytes.
// Always succeeds and returns nil error.
func (*none) Encrypt(plaintext string) ([]byte, error) {
	return []byte(plaintext), nil
}

// Decrypt performs no decryption and simply converts the bytes to a string.
// Always succeeds and returns nil error.
func (*none) Decrypt(ciphertext []byte) (string, error) {
	return string(ciphertext), nil
}
