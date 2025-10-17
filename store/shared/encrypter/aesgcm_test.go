package encrypter

import "testing"

func TestAesgcm(t *testing.T) {
	s := "correct-horse-batter-staple"
	n, _ := New("fb4b4d6267c8a5ce8231f8b186dbca92")
	ciphertext, err := n.Encrypt(s)
	if err != nil {
		t.Error(err)
	}
	plaintext, err := n.Decrypt(ciphertext)
	if err != nil {
		t.Error(err)
	}
	if want, got := plaintext, s; got != want {
		t.Errorf("Want plaintext %q, got %q", want, got)
	}
}

func TestAesgcmFail(t *testing.T) {
	s := "correct-horse-batter-staple"
	n, _ := New("ea1c5a9145c8a5ce8231f8b186dbcabc")
	ciphertext, err := n.Encrypt(s)
	if err != nil {
		t.Error(err)
	}
	n, _ = New("fb4b4d6267c8a5ce8231f8b186dbca92")
	_, err = n.Decrypt(ciphertext)
	if err == nil {
		t.Error("Expect error when encryption and decryption keys mismatch")
	}
}
