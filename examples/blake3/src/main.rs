use pyo3::interp::InterpreterBuilder;

mod extension {
    include!("lib.rs");
}

/// Upstream test vectors embedded at compile time.
const TEST_VECTORS_JSON: &str = include_str!("../../../blake3-vendor/tests/test_vectors.json");

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = extension::blake3_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).init_stdlib().build();

    let exit_code = interp.run(|vm| {
        // Inject the test vectors JSON as a global so Python can use it.
        let json_str = vm.ctx.new_str(TEST_VECTORS_JSON);
        let scope = vm.new_scope_with_builtins();
        scope
            .globals
            .set_item("_TEST_VECTORS_JSON", json_str.into(), vm)
            .unwrap();

        vm.run_block_expr(
            scope,
            r#"
import blake3 as blake3_mod
from blake3 import blake3, __version__
import json
import sys

_PASS = []
_FAIL = []

def run_test(name, fn):
    try:
        fn()
        _PASS.append(name)
        print(f"  PASS  {name}")
    except Exception as e:
        _FAIL.append(name)
        print(f"  FAIL  {name}: {type(e).__name__}: {e}")

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_input(length):
    b = bytearray(length)
    for i in range(len(b)):
        b[i] = i % 251
    return b

def unhexlify(hex_str):
    """Convert hex string to bytes (RustPython may not have binascii)."""
    return bytes(int(hex_str[i:i+2], 16) for i in range(0, len(hex_str), 2))

# ---------------------------------------------------------------------------
# test_vectors: core correctness tests from the upstream test vectors JSON
# ---------------------------------------------------------------------------
def test_vectors():
    VECTORS = json.loads(_TEST_VECTORS_JSON)
    cases = VECTORS["cases"]
    for case in cases:
        input_len = int(case["input_len"])
        input_bytes = make_input(input_len)
        extended_hash_hex = case["hash"]
        extended_keyed_hash_hex = case["keyed_hash"]
        extended_derive_key_hex = case["derive_key"]
        extended_hash_bytes = unhexlify(extended_hash_hex)
        extended_keyed_hash_bytes = unhexlify(extended_keyed_hash_hex)
        extended_derive_key_bytes = unhexlify(extended_derive_key_hex)
        hash_bytes = extended_hash_bytes[:32]
        keyed_hash_bytes = extended_keyed_hash_bytes[:32]
        derive_key_bytes = extended_derive_key_bytes[:32]
        extended_len = len(extended_hash_bytes)

        # default hash
        assert hash_bytes == blake3(input_bytes).digest(), f"hash mismatch at len={input_len}"
        assert extended_hash_bytes == blake3(input_bytes).digest(length=extended_len)
        assert extended_hash_hex == blake3(input_bytes).hexdigest(length=extended_len)
        incremental_hash = blake3()
        incremental_hash.update(input_bytes[: input_len // 2])
        incremental_hash.update(input_bytes[input_len // 2 :])
        assert hash_bytes == incremental_hash.digest()

        # keyed hash
        key = VECTORS["key"].encode()
        assert keyed_hash_bytes == blake3(input_bytes, key=key).digest()
        assert extended_keyed_hash_bytes == blake3(input_bytes, key=key).digest(length=extended_len)
        assert extended_keyed_hash_hex == blake3(input_bytes, key=key).hexdigest(length=extended_len)
        incremental_keyed_hash = blake3(key=key)
        incremental_keyed_hash.update(input_bytes[: input_len // 2])
        incremental_keyed_hash.update(input_bytes[input_len // 2 :])
        assert keyed_hash_bytes == incremental_keyed_hash.digest()

        # derive key
        context = "BLAKE3 2019-12-27 16:29:52 test vectors context"
        assert derive_key_bytes == blake3(input_bytes, derive_key_context=context).digest()
        assert extended_derive_key_bytes == blake3(input_bytes, derive_key_context=context).digest(length=extended_len)
        assert extended_derive_key_hex == blake3(input_bytes, derive_key_context=context).hexdigest(length=extended_len)
        incremental_derive_key = blake3(derive_key_context=context)
        incremental_derive_key.update(input_bytes[: input_len // 2])
        incremental_derive_key.update(input_bytes[input_len // 2 :])
        assert derive_key_bytes == incremental_derive_key.digest()

run_test("test_vectors", test_vectors)

# ---------------------------------------------------------------------------
# test_buffer_types: various buffer protocol inputs (bytearray, memoryview)
# ---------------------------------------------------------------------------
def test_buffer_types():
    expected = blake3(b"foo").digest()
    assert expected == blake3(bytearray(b"foo")).digest(), "bytearray input"
    assert expected == blake3(memoryview(b"foo")).digest(), "memoryview of bytes"
    assert expected == blake3(memoryview(bytearray(b"foo"))).digest(), "memoryview of bytearray"

    incremental = blake3()
    incremental.update(b"one")
    incremental.update(bytearray(b"two"))
    assert incremental.digest() == blake3(b"onetwo").digest()

run_test("test_buffer_types", test_buffer_types)

# ---------------------------------------------------------------------------
# test_key_types: key can be bytes, bytearray, memoryview
# ---------------------------------------------------------------------------
def test_key_types():
    key = bytes([42]) * 32
    expected = blake3(b"foo", key=key).digest()
    assert expected == blake3(b"foo", key=bytearray(key)).digest(), "bytearray key"
    assert expected == blake3(b"foo", key=memoryview(key)).digest(), "memoryview key"

run_test("test_key_types", test_key_types)

# ---------------------------------------------------------------------------
# test_invalid_key_lengths
# ---------------------------------------------------------------------------
def test_invalid_key_lengths():
    for key_length in range(0, 40):
        key = b"\xff" * key_length
        if key_length == blake3.key_size:
            blake3(b"foo", key=key)
        else:
            try:
                blake3(b"foo", key=key)
                assert False, f"should throw for key_length={key_length}"
            except ValueError:
                pass

run_test("test_invalid_key_lengths", test_invalid_key_lengths)

# ---------------------------------------------------------------------------
# test_constants
# ---------------------------------------------------------------------------
def test_constants():
    assert blake3.name == "blake3"
    assert blake3.digest_size == 32
    assert blake3.block_size == 64
    assert blake3.key_size == 32
    assert blake3.AUTO == -1
    assert blake3().name == "blake3"
    assert blake3().digest_size == 32
    assert blake3().block_size == 64
    assert blake3().key_size == 32
    assert blake3().AUTO == -1

run_test("test_constants", test_constants)

# ---------------------------------------------------------------------------
# test_xof: extended output function with length and seek
# ---------------------------------------------------------------------------
def test_xof():
    extended = blake3(b"foo").digest(length=100)
    for i in range(100):
        assert extended[:i] == blake3(b"foo").digest(length=i), f"prefix mismatch at i={i}"
        assert extended[i:] == blake3(b"foo").digest(length=100 - i, seek=i), f"seek mismatch at i={i}"

run_test("test_xof", test_xof)

# ---------------------------------------------------------------------------
# test_key_context_incompatible
# ---------------------------------------------------------------------------
def test_key_context_incompatible():
    zero_key = bytearray(32)
    try:
        blake3(b"foo", key=zero_key, derive_key_context="")
        assert False, "expected ValueError"
    except ValueError:
        pass

run_test("test_key_context_incompatible", test_key_context_incompatible)

# ---------------------------------------------------------------------------
# test_name
# ---------------------------------------------------------------------------
def test_name():
    b = blake3()
    assert b.name == "blake3"

run_test("test_name", test_name)

# ---------------------------------------------------------------------------
# test_copy_basic
# ---------------------------------------------------------------------------
def test_copy_basic():
    b = make_input(10**4)
    b2 = make_input(10**4)
    h1 = blake3(b)
    expected = h1.digest()
    h2 = h1.copy()
    assert expected == h2.digest(), "copy should produce same digest"
    h1.update(b2)
    expected2 = h1.digest()
    assert expected2 != h2.digest(), "copy should be independent"
    h2.update(b2)
    assert expected2 == h2.digest(), "update on copy should match original"

run_test("test_copy_basic", test_copy_basic)

# ---------------------------------------------------------------------------
# test_version
# ---------------------------------------------------------------------------
def test_version():
    assert type(__version__) is str, "version should be a string"
    assert len(__version__.split(".")) == 3, f"version should have 3 parts: {__version__}"

run_test("test_version", test_version)

# ---------------------------------------------------------------------------
# test_reset
# ---------------------------------------------------------------------------
def test_reset():
    hasher = blake3()
    hash1 = hasher.digest()
    hasher.update(b"foo")
    hash2 = hasher.digest()
    hasher.reset()
    hash3 = hasher.digest()
    hasher.update(b"foo")
    hash4 = hasher.digest()
    assert hash1 != hash2
    assert hash1 == hash3, "after reset, digest should be same as empty"
    assert hash2 == hash4, "after reset+update, digest should match"

run_test("test_reset", test_reset)

# ---------------------------------------------------------------------------
# test_output_overflows_isize: OverflowError for too-large length
# ---------------------------------------------------------------------------
def test_output_overflows_isize():
    try:
        blake3().digest(sys.maxsize + 1)
        assert False, "should throw"
    except (OverflowError, MemoryError, ValueError):
        pass
    try:
        blake3().hexdigest((sys.maxsize // 2) + 1)
        assert False, "should throw"
    except (OverflowError, MemoryError, ValueError):
        pass

run_test("test_output_overflows_isize", test_output_overflows_isize)

# ---------------------------------------------------------------------------
# test_usedforsecurity_ignored: hashlib compat kwarg is accepted but ignored
# ---------------------------------------------------------------------------
def test_usedforsecurity_ignored():
    blake3(usedforsecurity=True)
    blake3(usedforsecurity=False)

run_test("test_usedforsecurity_ignored", test_usedforsecurity_ignored)

# ---------------------------------------------------------------------------
# test_context_must_be_str
# ---------------------------------------------------------------------------
def test_context_must_be_str():
    blake3(derive_key_context="foo")  # string works
    try:
        blake3(derive_key_context=b"foo")  # bytes should fail
        assert False, "should fail"
    except TypeError:
        pass

run_test("test_context_must_be_str", test_context_must_be_str)

# ---------------------------------------------------------------------------
# test_buffers_released (CBuffer protocol: buffer released after hashing)
# ---------------------------------------------------------------------------
def test_buffers_released():
    key = bytearray(32)
    message = bytearray(32)
    hasher = blake3(message, key=key)
    hasher.update(message)
    # If buffers are properly released, these extend() calls should work.
    key.extend(b"foo")
    message.extend(b"foo")
    assert len(key) == 35
    assert len(message) == 35

run_test("test_buffers_released", test_buffers_released)

# ---------------------------------------------------------------------------
# test_mmap: mmap file hashing
# ---------------------------------------------------------------------------
def test_mmap():
    import os
    import tempfile
    input_bytes = bytes([42]) * 10_000
    (fd, temp_path) = tempfile.mkstemp()
    os.close(fd)
    try:
        with open(temp_path, "wb") as f:
            f.write(input_bytes)

        hasher1 = blake3()
        hasher1.update_mmap(temp_path)
        assert blake3(input_bytes).digest() == hasher1.digest(), "mmap digest should match"

        # update_mmap with keyword argument
        hasher2 = blake3()
        hasher2.update_mmap(path=temp_path)
        assert blake3(input_bytes).digest() == hasher2.digest(), "mmap keyword arg"

        # nonexistent file raises OSError
        try:
            hasher1.update_mmap("/non/existent/file.txt")
            assert False, "expected error for nonexistent file"
        except (FileNotFoundError, OSError):
            pass
    finally:
        os.unlink(temp_path)

run_test("test_mmap", test_mmap)

# ---------------------------------------------------------------------------
# test_max_threads: max_threads kwarg in constructor
# ---------------------------------------------------------------------------
def test_max_threads():
    b = make_input(10**4)
    expected = blake3(b).digest()
    assert expected == blake3(b, max_threads=1).digest(), "max_threads=1"
    assert expected == blake3(b, max_threads=2).digest(), "max_threads=2"
    assert expected == blake3(b, max_threads=blake3.AUTO).digest(), "max_threads=AUTO"

run_test("test_max_threads", test_max_threads)

# ---------------------------------------------------------------------------
# test_invalid_max_threads
# ---------------------------------------------------------------------------
def test_invalid_max_threads():
    try:
        blake3(max_threads=0)
        assert False, "expected ValueError for max_threads=0"
    except ValueError:
        pass
    try:
        blake3(max_threads=-2)
        assert False, "expected ValueError for max_threads=-2"
    except ValueError:
        pass

run_test("test_invalid_max_threads", test_invalid_max_threads)

# ---------------------------------------------------------------------------
# test_int_array_fails (CBuffer: array of int should fail)
# ---------------------------------------------------------------------------
def test_int_array_fails():
    import array as array_mod
    try:
        blake3(array_mod.array("i"))
        assert False, "expected error for int array"
    except (BufferError, ValueError, TypeError):
        pass
    try:
        blake3().update(array_mod.array("i"))
        assert False, "expected error for int array in update"
    except (BufferError, ValueError, TypeError):
        pass

run_test("test_int_array_fails", test_int_array_fails)

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
print()
print(f"Results: {len(_PASS)} passed, {len(_FAIL)} failed")
if _FAIL:
    print(f"Failed: {_FAIL}")
    import sys
    sys.exit(1)
else:
    print("All tests passed!")
"#,
        )
        .map(|_| ())
    });

    if exit_code != 0 {
        std::process::exit(exit_code as i32);
    }
}
