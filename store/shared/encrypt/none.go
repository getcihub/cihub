package encrypt

// none is an Encrypter that stores secret values
// in plain text.
type none struct{}

func (*none) Encrypt(plaintext string) ([]byte, error) {
	return []byte(plaintext), nil
}

func (*none) Decrypt(ciphertext []byte) (string, error) {
	return string(ciphertext), nil
}
