import Testing
@testable import BinderCore

/// The sample secrets are assembled at run time, so no literal token sits in the repo
/// for GitHub's push protection (or anyone) to trip over.
private enum Corpus {
    static let alnum36 = String(repeating: "a1B2c3D4e5", count: 4).prefix(36)
    static let githubToken = "gh" + "p_" + alnum36

    static let secrets: [String] = [
        "-----BEGIN " + "OPENSSH PRIVATE KEY-----",
        "-----BEGIN " + "RSA PRIVATE KEY-----",
        "aws_access_key_id = " + "AKIA" + "IOSFODNN7EXAMPLE",
        githubToken,
        "github_" + "pat_" + "11ABCDEFG0123456789_abcdefghijklmnop",
        "sk-" + "ant-" + "api03-" + "q7Rk2LmP9vT4wXy8Bn3Hj6Fd1Sg5Ac0",
        "sk-" + "proj-" + "Zx9q7Rk2LmP9vT4wXy8Bn3Hj6",
        "xox" + "b-" + "1234567890-abcdefghijKLM",
        "export GITHUB_" + "TOKEN=" + "s3cretValue99",
        "password: " + "hunter22x",
        "DB_PASS" + "WORD=" + "correcthorse7",
        "secret_key = " + "abcdef12345",
        "git clone https://eren:" + "hunter2pass@github.com/Kungie/dawn",
        "Authorization: Bearer " + "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9abc",
        "value " + "q7Rk2LmP9vT4wXy8Bn3Hj6Fd1Sg5Ac0Ze7Ur2Ki9"
    ]

    static let clean: [String] = [
        "commit 2cad5b4f0e9a1b3c5d7e9f1a3b5c7d9e1f3a5b7c",
        "session 8864d7f4-9dbc-4a10-a14c-3a41cdef7a16",
        "hetzner-1 opens with id_ed25519_hetzner. No new key needed.",
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGq3Xb1Lf8Kq9Zt2Wm4Rn7Vp0Ys6Hd5Jc8Ue3Ta1Fo eren@mac",
        "key id_ed25519: ED25519 SHA256:q7Rk2LmP9vT4wXy8Bn3Hj6Fd1Sg5Ac0Ze7Ur2Ki9Xw4",
        "Read ~/.ssh/config and /Users/erenmac/Library/Caches/Binder/build/arm64-apple-macosx2024",
        "max_tokens: 600",
        "context_tokens = 341470",
        "The dawn site deploys to Cloudflare Pages.",
        "https://github.com/Kungie/dawn and git@github.com:Kungie/dawn.git",
        "BinderCoreTestsSecretFilterTestsEverySampleSecretIsCaught",
        "Tokens used: 412 of 2400. Password rules are in docs/security.md."
    ]
}

@Suite struct SecretFilterTests {
    @Test func everySampleSecretIsCaught() {
        for sample in Corpus.secrets {
            #expect(!SecretFilter.findings(in: sample).isEmpty, "missed: \(sample)")
            #expect(SecretFilter.containsSecret(sample))
        }
    }

    @Test func nothingInCleanCorpusIsCaught() {
        for sample in Corpus.clean {
            #expect(SecretFilter.findings(in: sample).isEmpty, "false alarm: \(sample) → \(SecretFilter.findings(in: sample))")
        }
    }

    @Test func redactKeepsSurroundingText() {
        let command = "export GITHUB_TOKEN=\(Corpus.githubToken) && vercel deploy --prod"
        let redacted = SecretFilter.redact(command)
        #expect(!redacted.contains(Corpus.githubToken))
        #expect(redacted.hasPrefix("export GITHUB_TOKEN=[REDACTED:"))
        #expect(redacted.hasSuffix("] && vercel deploy --prod"))
        #expect(!SecretFilter.containsSecret(redacted))
        #expect(SecretFilter.redact("nothing to see") == "nothing to see")
    }
}
