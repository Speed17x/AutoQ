package com.speed17x.trivium.hardware

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothAdapter
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.BluetoothStatusCodes
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat
import com.speed17x.trivium.dsp.FilterBand
import com.speed17x.trivium.dsp.FilterType
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.withTimeoutOrNull
import java.util.UUID

class BleDspException(message: String, cause: Throwable? = null) : RuntimeException(message, cause)

/**
 * Native Android Bluetooth LE GATT transport for hardware PEQ headphones/DACs:
 * - FiiO EH11 / EH13 (`00001100-04a5-1000-1000-40ed981a04a5`)
 * - Airoha / Audeze Maxwell (`5052494d-2dab-0341-6972-6f6861424c45`)
 */
class BleDspTransport(private val context: Context) {

    companion object {
        private val CCCD_UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
    }

    private val btManager: BluetoothManager?
        get() = context.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager

    private val adapter: BluetoothAdapter?
        get() = btManager?.adapter

    fun hasBlePermissions(): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val c = ContextCompat.checkSelfPermission(context, Manifest.permission.BLUETOOTH_CONNECT)
            return c == PackageManager.PERMISSION_GRANTED
        }
        return true
    }

    @SuppressLint("MissingPermission")
    fun scanKnownBleDevices(): List<MoondropDevice> {
        if (!hasBlePermissions()) return emptyList()
        val mgr = btManager ?: return emptyList()
        val adp = adapter ?: return emptyList()
        if (!adp.isEnabled) return emptyList()

        val candidates = LinkedHashMap<String, BluetoothDevice>()
        try {
            for (d in mgr.getConnectedDevices(BluetoothProfile.GATT)) {
                candidates[d.address] = d
            }
        } catch (_: SecurityException) {
        }
        try {
            for (d in adp.bondedDevices.orEmpty()) {
                candidates.putIfAbsent(d.address, d)
            }
        } catch (_: SecurityException) {
        }

        val out = mutableListOf<MoondropDevice>()
        for ((addr, dev) in candidates) {
            val name = try {
                dev.name ?: ""
            } catch (_: SecurityException) {
                ""
            }
            val classified = MoondropDeviceCatalog.classifyBleDevice(name, addr)
            if (classified != null) {
                out += classified
            }
        }
        return out.sortedBy { it.displayName }
    }

    @SuppressLint("MissingPermission")
    suspend fun readEq(info: MoondropDevice): Pair<Boolean, List<FilterBand>> {
        val (svcUuid, txUuid, rxUuid) = gattUuidsFor(info.protocol)
        return withGattSession(info.btMac, svcUuid, txUuid, rxUuid) { session ->
            when (info.protocol) {
                DspProtocol.FIIO_BLE -> {
                    session.drainNotifications()
                    runCatching {
                        session.writePacket(MoondropProtocolCodec.fiioBleHandshakePacket())
                        session.readChunk(1200)
                    }
                    session.writePacket(MoondropProtocolCodec.fiioBleReadEqPacket())
                    val acc = ArrayList<Byte>(96)
                    val deadline = System.currentTimeMillis() + 6000L
                    while (System.currentTimeMillis() < deadline) {
                        val rem = (deadline - System.currentTimeMillis()).coerceAtLeast(100L)
                        val chunk = session.readChunk(rem) ?: break
                        for (b in chunk) acc.add(b)
                        if (acc.size >= 4) {
                            val expected = ((acc[2].toInt() and 0xFF) shl 8) or (acc[3].toInt() and 0xFF)
                            if (acc.size >= expected) {
                                val parsed = MoondropProtocolCodec.fiioBleParseEqResponse(acc.toByteArray())
                                    ?: throw BleDspException("Respuesta FiiO BLE inválida (${acc.size} B).")
                                return@withGattSession true to parsed
                            }
                        }
                    }
                    throw BleDspException("Timeout leyendo curva PEQ por FiiO BLE GATT.")
                }

                DspProtocol.AIROHA_BLE -> {
                    session.drainNotifications()
                    session.writePacket(MoondropProtocolCodec.airohaBleReadPresetPacket(info.userSlot))
                    val acc = ArrayList<Byte>(256)
                    val deadline = System.currentTimeMillis() + 5000L
                    while (System.currentTimeMillis() < deadline) {
                        val rem = (deadline - System.currentTimeMillis()).coerceAtLeast(100L)
                        val chunk = session.readChunk(rem) ?: break
                        for (b in chunk) acc.add(b)
                        val hdrIdx = acc.indexOf(0x05.toByte())
                        if (hdrIdx > 0) {
                            repeat(hdrIdx) { acc.removeAt(0) }
                        }
                        if (acc.size >= MoondropProtocolCodec.AIROHA_BLE_RESPONSE_LEN) {
                            val parsed = MoondropProtocolCodec.airohaBleParsePeqResponse(acc.toByteArray())
                            if (parsed != null) return@withGattSession parsed
                            acc.removeAt(0)
                        }
                    }
                    throw BleDspException("Timeout leyendo curva PEQ por Airoha BLE GATT.")
                }

                else -> throw BleDspException("Protocolo no BLE GATT: ${info.protocol}")
            }
        }
    }

    @SuppressLint("MissingPermission")
    suspend fun writeEq(info: MoondropDevice, bands: List<FilterBand>, enableEq: Boolean = true) {
        val fitted = MutableList(info.maxBands) { i ->
            val src = bands.getOrNull(i) ?: FilterBand(FilterType.PEAKING, 1000.0, 0.0, 1.0)
            if (enableEq) {
                src.clamped(maxGainDb = info.maxGainDb, allowShelves = info.supportsShelves)
            } else {
                FilterBand(FilterType.PEAKING, src.frequency, 0.0, 1.0)
            }
        }
        val (svcUuid, txUuid, rxUuid) = gattUuidsFor(info.protocol)
        withGattSession(info.btMac, svcUuid, txUuid, rxUuid) { session ->
            when (info.protocol) {
                DspProtocol.FIIO_BLE -> {
                    for (i in 0 until 10) {
                        val b = fitted.getOrElse(i) { FilterBand() }
                        val pkt = MoondropProtocolCodec.fiioBleWriteBandPacket(i, b)
                        session.writePacket(pkt)
                        session.readChunk(500)
                        kotlinx.coroutines.delay(50)
                    }
                }

                DspProtocol.AIROHA_BLE -> {
                    val pkt = MoondropProtocolCodec.airohaBleWritePeqPacket(info.userSlot, fitted)
                    session.writePacket(pkt)
                }

                else -> throw BleDspException("Protocolo no BLE GATT: ${info.protocol}")
            }
        }
    }

    private fun gattUuidsFor(proto: DspProtocol): Triple<UUID, UUID, UUID> = when (proto) {
        DspProtocol.FIIO_BLE -> Triple(
            UUID.fromString(MoondropProtocolCodec.FIIO_BLE_SERVICE_UUID),
            UUID.fromString(MoondropProtocolCodec.FIIO_BLE_TX_UUID),
            UUID.fromString(MoondropProtocolCodec.FIIO_BLE_RX_UUID),
        )
        DspProtocol.AIROHA_BLE -> Triple(
            UUID.fromString(MoondropProtocolCodec.AIROHA_BLE_SERVICE_UUID),
            UUID.fromString(MoondropProtocolCodec.AIROHA_BLE_TX_UUID),
            UUID.fromString(MoondropProtocolCodec.AIROHA_BLE_RX_UUID),
        )
        else -> throw BleDspException("Protocolo BLE no soportado: $proto")
    }

    private class GattSession(
        private val gatt: BluetoothGatt,
        private val txChar: BluetoothGattCharacteristic,
        private val rxNotifications: Channel<ByteArray>,
        private val writeDoneSignal: CompletableDeferred<Int>,
    ) {
        fun drainNotifications() {
            while (rxNotifications.tryReceive().isSuccess) {
            }
        }

        suspend fun readChunk(timeoutMs: Long): ByteArray? = withTimeoutOrNull(timeoutMs) {
            rxNotifications.receive()
        }

        @SuppressLint("MissingPermission")
        suspend fun writePacket(bytes: ByteArray) {
            val useWithResponse =
                (txChar.properties and BluetoothGattCharacteristic.PROPERTY_WRITE) != 0 &&
                    (txChar.properties and BluetoothGattCharacteristic.PROPERTY_WRITE_NO_RESPONSE) == 0
            val writeType = if (useWithResponse) {
                BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT
            } else {
                BluetoothGattCharacteristic.WRITE_TYPE_NO_RESPONSE
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                val st = gatt.writeCharacteristic(txChar, bytes, writeType)
                if (st != BluetoothStatusCodes.SUCCESS) {
                    throw BleDspException("Error GATT writeCharacteristic ($st).")
                }
            } else {
                @Suppress("DEPRECATION")
                txChar.writeType = writeType
                @Suppress("DEPRECATION")
                txChar.value = bytes
                @Suppress("DEPRECATION")
                if (!gatt.writeCharacteristic(txChar)) {
                    throw BleDspException("Error enviando paquete BLE GATT.")
                }
            }
            kotlinx.coroutines.delay(25)
        }
    }

    @SuppressLint("MissingPermission")
    private suspend fun <T> withGattSession(
        mac: String,
        serviceUuid: UUID,
        txUuid: UUID,
        rxUuid: UUID,
        block: suspend (GattSession) -> T,
    ): T {
        if (!hasBlePermissions()) {
            throw BleDspException("Se requiere permiso BLUETOOTH_CONNECT para operar por BLE GATT.")
        }
        val adp = adapter ?: throw BleDspException("Adaptador Bluetooth no disponible.")
        val remote = try {
            adp.getRemoteDevice(mac)
        } catch (e: Exception) {
            throw BleDspException("Dirección BLE inválida ($mac): ${e.message}", e)
        }

        val ready = CompletableDeferred<Result<BluetoothGatt>>()
        val rxChannel = Channel<ByteArray>(Channel.UNLIMITED)
        var writeDeferred = CompletableDeferred<Int>()

        val callback = object : BluetoothGattCallback() {
            override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
                if (status != BluetoothGatt.GATT_SUCCESS) {
                    ready.complete(Result.failure(BleDspException("Conexión BLE fallida (status $status).")))
                    return
                }
                if (newState == BluetoothProfile.STATE_CONNECTED) {
                    if (!gatt.discoverServices()) {
                        ready.complete(Result.failure(BleDspException("No se pudieron descubrir servicios GATT.")))
                    }
                } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                    rxChannel.close()
                }
            }

            override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
                if (status != BluetoothGatt.GATT_SUCCESS) {
                    ready.complete(Result.failure(BleDspException("Error descubriendo servicios GATT ($status).")))
                } else {
                    ready.complete(Result.success(gatt))
                }
            }

            override fun onCharacteristicChanged(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                value: ByteArray,
            ) {
                rxChannel.trySend(value.copyOf())
            }

            @Deprecated("Deprecated in Java")
            override fun onCharacteristicChanged(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
            ) {
                @Suppress("DEPRECATION")
                val v = characteristic.value ?: return
                rxChannel.trySend(v.copyOf())
            }

            override fun onCharacteristicWrite(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                status: Int,
            ) {
                writeDeferred.complete(status)
            }
        }

        val gatt = remote.connectGatt(context, false, callback, BluetoothDevice.TRANSPORT_LE)
            ?: throw BleDspException("No se pudo iniciar conexión BLE GATT.")

        return try {
            val res = withTimeoutOrNull(8000L) { ready.await() }
                ?: throw BleDspException("Timeout conectando con dispositivo BLE GATT ($mac).")
            val connectedGatt = res.getOrThrow()
            val service = connectedGatt.getService(serviceUuid)
                ?: throw BleDspException("El dispositivo BLE no expone el servicio PEQ esperado.")
            val txChar = service.getCharacteristic(txUuid)
                ?: throw BleDspException("Característica BLE TX no encontrada.")
            val rxChar = service.getCharacteristic(rxUuid)
                ?: throw BleDspException("Característica BLE RX no encontrada.")

            connectedGatt.setCharacteristicNotification(rxChar, true)
            val cccd = rxChar.getDescriptor(CCCD_UUID)
            if (cccd != null) {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    connectedGatt.writeDescriptor(cccd, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
                } else {
                    @Suppress("DEPRECATION")
                    cccd.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
                    @Suppress("DEPRECATION")
                    connectedGatt.writeDescriptor(cccd)
                }
                kotlinx.coroutines.delay(120)
            }

            val session = GattSession(connectedGatt, txChar, rxChannel, writeDeferred)
            block(session)
        } finally {
            try {
                gatt.disconnect()
            } catch (_: Exception) {
            }
            try {
                gatt.close()
            } catch (_: Exception) {
            }
            rxChannel.close()
        }
    }
}
