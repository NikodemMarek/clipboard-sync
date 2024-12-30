package com.example.clipboardsync

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context.CLIPBOARD_SERVICE
import android.os.Bundle
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.ClickableText
import androidx.compose.material3.Button
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import androidx.work.WorkManager
import com.example.clipboardsync.ui.theme.ClipboardSyncTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.PublicKey
import java.security.spec.X509EncodedKeySpec
import javax.crypto.Cipher
import kotlin.io.encoding.Base64
import kotlin.io.encoding.ExperimentalEncodingApi

object KeyStoreUtils {
    private val ANDROID_KEY_STORE = "AndroidKeyStore"
    private val alias = "clipboard-sync-key"

    private val keyStore = KeyStore.getInstance(ANDROID_KEY_STORE).apply {
        load(null)
    }

    fun getKeyPair(): KeyPair {
        val entry = keyStore.getEntry(alias, null)
        return if (entry == null) {
            createKeyPair()
        } else {
            val privateKey = (entry as KeyStore.PrivateKeyEntry).privateKey
            val publicKey = keyStore.getCertificate(alias).publicKey
            KeyPair(publicKey, privateKey)
        }
    }

    private fun createKeyPair(): KeyPair {
        val keyGenerator = KeyPairGenerator
            .getInstance(KeyProperties.KEY_ALGORITHM_RSA, ANDROID_KEY_STORE)

        keyGenerator.initialize(
            KeyGenParameterSpec.Builder(
                alias,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setDigests(KeyProperties.DIGEST_SHA256)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_RSA_PKCS1)
                .setKeySize(2048)
                .build()
        )

        return keyGenerator.generateKeyPair()
    }

    fun removeKey() {
        keyStore.deleteEntry(alias)
    }
}

class PeersData(clients: List<String> = emptyList()) {
    private val kf = KeyFactory.getInstance("RSA")
    private val cipher = Cipher.getInstance("RSA/ECB/PKCS1Padding")

    private val clients = mutableMapOf<String, PublicKey>()

    init {
        clients.forEach { addClient(it) }
    }

    fun addClient(rawKey: String): String {
        val pubKey = kf.generatePublic(X509EncodedKeySpec(stringToByteArray(rawKey))) as PublicKey
        val id = digest(pubKey.encoded)
        clients[id] = pubKey

        return id
    }

    fun removeClient(id: String): PublicKey? {
        return clients.remove(id)
    }

    val keys: Array<PublicKey>
        get() {
            return clients.values.toTypedArray()
        }
    val ids: Array<String>
        get() {
            return clients.keys.toTypedArray()
        }

    fun getPublicKey(id: String): PublicKey? {
        return clients[id]
    }

    private fun encryptMessage(message: String, id: String): ByteArray {
        cipher.init(Cipher.ENCRYPT_MODE, clients[id])
        return cipher.doFinal(message.toByteArray())
    }

    fun encryptForAll(message: String): Map<String, ByteArray> {
        return clients.mapValues { (id, _) ->
            encryptMessage(message, id)
        }
    }
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        var keyPair = KeyStoreUtils.getKeyPair()
        val workManager = WorkManager.getInstance(this)
        val db = MainDatabase.init(this)
        val peersData = PeersData()

        lifecycleScope.launch {
            withContext(Dispatchers.IO) {

                db?.peerDao()?.getAll()?.forEach {
                    peersData.addClient(it.key)
                }
            }
        }

        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            ClipboardSyncTheme {
                Scaffold(
                    modifier = Modifier
                        .padding(16.dp)
                        .fillMaxSize()
                ) { innerPadding ->
                    Column(modifier = Modifier.padding(innerPadding)) {
                        KeyDisplay(
                            key = keyPair.public,
                        )

                        Button(onClick = {
                            val request = SyncWorker.create(peersData)
                            workManager.enqueue(request)
                        }) {
                            Text(text = "start")
                        }

                        Button(onClick = {
                            KeyStoreUtils.removeKey()
                            keyPair = KeyStoreUtils.getKeyPair()
                        }) {
                            Text(text = "regenerate key")
                        }

                        if (db != null) {
                            ClientKeys(peersData = peersData)
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalEncodingApi::class)
fun keyToString(key: PublicKey): String =
    Base64.encode(key.encoded)

@OptIn(ExperimentalEncodingApi::class)
fun stringToByteArray(str: String): ByteArray =
    Base64.decode(str)

fun keyStringToPem(keyString: String): String =
    "-----BEGIN PUBLIC KEY-----\n" + keyString.chunked(64)
        .joinToString("\n") + "\n-----END PUBLIC KEY-----"

fun pemToKeyString(pem: String): String =
    pem
        .replace(" ", "")
        .replace("\n", "")
        .replace("-----BEGINPUBLICKEY-----", "")
        .replace("-----ENDPUBLICKEY-----", "")
        .replace("'", "")

fun digest(key: ByteArray): String =
    bin2hex(MessageDigest.getInstance("SHA-256").digest(key))

fun bin2hex(data: ByteArray): String {
    val hex = StringBuilder(data.size * 2)
    for (b in data) hex.append(String.format("%02x", b.toInt() and 0xFF))
    return hex.toString()
}

@Composable
fun KeyDisplay(key: PublicKey) {
    val clipboardManager =
        requireNotNull(LocalContext.current.getSystemService(CLIPBOARD_SERVICE)) as ClipboardManager

    val pem = keyStringToPem(keyToString(key))

    ClickableText(text = AnnotatedString(pem)) {
        clipboardManager.setPrimaryClip(ClipData.newPlainText("", pem))
    }

    Text(text = digest(key.encoded))
}

@Composable
fun ClientKeys(peersData: PeersData) {
    var newKeyValue by remember { mutableStateOf("") }

    val scope = rememberCoroutineScope()
    val peerDao = MainDatabase.instance!!.peerDao()

    TextField(value = newKeyValue, onValueChange = {
        newKeyValue = it
    })

    Button(onClick = {
        val id = peersData.addClient(pemToKeyString(newKeyValue))
        newKeyValue = ""

        val key = peersData.getPublicKey(id)!!
        scope.launch {
            withContext(Dispatchers.IO) {
                peerDao.insert(Peer(key = keyToString(key)))
            }
        }
    }) {
        Text(text = "add key")
    }

    peersData.ids.forEach { id ->
        Row {
            Text(text = id)
            Button(onClick = {
                val key = peersData.removeClient(id)

                scope.launch {
                    withContext(Dispatchers.IO) {
                        peerDao.delete(Peer(key = keyToString(key!!)))
                    }
                }
            }) {
                Text(text = "remove")
            }
        }
    }
}