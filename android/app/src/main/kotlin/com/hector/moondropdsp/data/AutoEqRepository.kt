package com.hector.moondropdsp.data

import android.content.Context
import com.hector.moondropdsp.dsp.BiquadMath
import com.hector.moondropdsp.dsp.FilterBand
import com.hector.moondropdsp.dsp.FilterType
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLDecoder
import java.security.MessageDigest
import kotlin.math.abs
import kotlin.math.roundToInt

@Serializable
enum class EntrySourceKind {
    AUTOEQ_GITHUB,
    SQUIGLINK,
}

data class SquigSourceSpec(
    val id: String,
    val label: String,
    val baseUrl: String,
    val defaultFormFactor: String = "in-ear",
    val rigTag: String = "711",
)

@Serializable
data class AutoEqEntry(
    val name: String,
    val relPath: String,
    val sourceRig: String,
    val sourceKind: EntrySourceKind = EntrySourceKind.AUTOEQ_GITHUB,
    val squigBaseUrl: String = "",
    val squigFileStem: String = "",
    val formFactorOverride: String = "",
    val rigTagOverride: String = "",
) {
    val uniqueKey: String
        get() = if (sourceKind == EntrySourceKind.SQUIGLINK) "sq:$relPath" else "ae:$relPath"

    val formFactor: String
        get() {
            if (formFactorOverride.isNotBlank()) return formFactorOverride
            val low = try {
                URLDecoder.decode(relPath, "UTF-8").lowercase()
            } catch (_: Exception) {
                relPath.lowercase()
            }
            return when {
                "in-ear" in low -> "in-ear"
                "over-ear" in low -> "over-ear"
                "earbud" in low -> "earbud"
                else -> "in-ear"
            }
        }

    val rigTag: String
        get() {
            if (rigTagOverride.isNotBlank()) return rigTagOverride
            val low = sourceRig.lowercase()
            return when {
                "5128" in low -> "5128"
                "4620" in low -> "4620"
                "hms" in low -> "HMS"
                "711" in low || "super* review" in low || "super review" in low ||
                    "oratory1990" in low || "tonedeafmonk" in low || "aftersound" in low ||
                    "hbb" in low || "paul wasabii" in low || "jaytiss" in low || "regancipher" in low -> "711"
                else -> sourceRig
            }
        }

    val label: String get() = "$name — $sourceRig"

    val folderSegment: String
        get() = relPath.substringAfterLast('/')

    val graphicEqUrl: String
        get() = "${AutoEqParser.RESULTS_BASE_URL}/$relPath/${folderSegment}%20GraphicEQ.txt"

    val parametricEqUrl: String
        get() = "${AutoEqParser.RESULTS_BASE_URL}/$relPath/${folderSegment}%20ParametricEQ.txt"
}

data class CuratedTarget(
    val title: String,
    val csvFileName: String?,
)

data class CatalogLoadSummary(
    val entries: List<AutoEqEntry>,
    val activeSourceCount: Int,
    val totalSourceCount: Int,
)

data class AutoEqResult(
    val bands: List<FilterBand>,
    val rmseDb: Double,
    val warning: String?,
    val targetLabel: String,
    val stockFrCurve: List<CurvePoint>,
    val targetFrCurve: List<CurvePoint>,
)

object AutoEqParser {
    const val INDEX_URL = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results/INDEX.md"
    const val RESULTS_BASE_URL = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/results"
    const val TARGETS_BASE_URL = "https://raw.githubusercontent.com/jaakkopasanen/AutoEq/master/targets"

    /** Verified open public Squiglink measurement repositories (JSON index + raw FR .txt). */
    val SQUIG_SOURCES: List<SquigSourceSpec> = listOf(
        SquigSourceSpec("superreview", "Super* Review (711)", "https://squig.link/data/", "in-ear", "711"),
        SquigSourceSpec("regancipher", "ReganCipher (TWS/711)", "https://regancipher.squig.link/data/", "in-ear", "711"),
        SquigSourceSpec("aftersound", "Aftersound (711)", "https://aftersound.squig.link/data/", "in-ear", "711"),
        SquigSourceSpec("hbb", "HBB (711)", "https://hbb.squig.link/data/", "in-ear", "711"),
        SquigSourceSpec("paulwasabii", "Paul Wasabii (711)", "https://pw.squig.link/data/", "in-ear", "711"),
        SquigSourceSpec("jaytiss", "Jaytiss (711)", "https://jaytiss.squig.link/data/", "in-ear", "711"),
        SquigSourceSpec("gadgetrytech", "GadgetryTech (Over-Ear)", "https://gadgetrytech.squig.link/headsets/data/", "over-ear", "over-ear"),
    )

    val CURATED_TARGETS = listOf(
        CuratedTarget("AutoEq In-Ear (Predeterminado)", null),
        CuratedTarget("Harman In-Ear 2019", "Harman in-ear 2019.csv"),
        CuratedTarget("Harman In-Ear 2019 (sin boost bajos)", "Harman in-ear 2019 without bass.csv"),
        CuratedTarget("Harman In-Ear 2017-2", "Harman in-ear 2017-2.csv"),
        CuratedTarget("Harman Over-Ear 2018", "Harman over-ear 2018.csv"),
        CuratedTarget("JM-1 con filtros Harman", "JM-1 with Harman filters.csv"),
        CuratedTarget("Diffuse Field 5128 (-1 dB/oct)", "Diffuse field 5128 -1dB per octave.csv"),
        CuratedTarget("Oratory1990 In-Ear", "oratory1990 in-ear.csv"),
    )

    private val INDEX_RE = Regex("""^- \[(.+?)]\(\./(.+?)\) by (.+)$""")
    private val PARAM_RE = Regex(
        """^Filter\s+\d+:\s+ON\s+(PK|LSC|HSC)\s+Fc\s+([\d.]+)\s+Hz\s+Gain\s+([+-]?[\d.]+)\s+dB\s+Q\s+([\d.]+)""",
        RegexOption.IGNORE_CASE,
    )
    private val jsonParser = Json { ignoreUnknownKeys = true; isLenient = true }

    fun encodeUrlPathSegment(segment: String): String {
        val bytes = segment.toByteArray(Charsets.UTF_8)
        val sb = StringBuilder(bytes.size + 16)
        for (b in bytes) {
            val c = b.toInt() and 0xFF
            val unreserved = (c in 'A'.code..'Z'.code) ||
                (c in 'a'.code..'z'.code) ||
                (c in '0'.code..'9'.code) ||
                c == '-'.code || c == '_'.code || c == '.'.code || c == '~'.code
            if (unreserved) {
                sb.append(c.toChar())
            } else {
                sb.append('%')
                sb.append("0123456789ABCDEF"[c ushr 4])
                sb.append("0123456789ABCDEF"[c and 0x0F])
            }
        }
        return sb.toString()
    }

    fun parseIndexMarkdown(text: String): List<AutoEqEntry> {
        val entries = ArrayList<AutoEqEntry>(8900)
        for (rawLine in text.lineSequence()) {
            val m = INDEX_RE.matchEntire(rawLine.trim()) ?: continue
            entries += AutoEqEntry(
                name = m.groupValues[1],
                relPath = m.groupValues[2],
                sourceRig = m.groupValues[3],
                sourceKind = EntrySourceKind.AUTOEQ_GITHUB,
            )
        }
        return entries
    }

    fun parseSquigPhoneBook(jsonText: String, source: SquigSourceSpec): List<AutoEqEntry> {
        val root = try {
            jsonParser.parseToJsonElement(jsonText).jsonArray
        } catch (_: Exception) {
            return emptyList()
        }

        val entries = ArrayList<AutoEqEntry>(512)
        for (brandEl in root) {
            val brandObj = brandEl as? JsonObject ?: continue
            val brandName = (brandObj["name"] as? JsonPrimitive)?.content?.trim().orEmpty()
            val phones = brandObj["phones"] as? JsonArray ?: continue

            for (phEl in phones) {
                val phObj = phEl as? JsonObject ?: continue
                val phoneName = (phObj["name"] as? JsonPrimitive)?.content?.trim().orEmpty()
                if (phoneName.isBlank()) continue

                val baseFullName = if (brandName.isNotBlank() && !phoneName.lowercase().startsWith(brandName.lowercase())) {
                    "$brandName $phoneName".trim()
                } else {
                    phoneName
                }

                val fileEl = phObj["file"] ?: continue
                val files: List<String> = when (fileEl) {
                    is JsonPrimitive -> listOfNotNull(fileEl.content.takeIf { it.isNotBlank() })
                    is JsonArray -> fileEl.mapNotNull { (it as? JsonPrimitive)?.content?.takeIf { s -> s.isNotBlank() } }
                    else -> emptyList()
                }
                if (files.isEmpty()) continue

                val suffixes: List<String> = when (val sEl = phObj["suffix"]) {
                    is JsonArray -> sEl.map { (it as? JsonPrimitive)?.content?.trim().orEmpty() }
                    is JsonPrimitive -> listOf(sEl.content.trim())
                    else -> emptyList()
                }

                files.forEachIndexed { idx, fileStem ->
                    val suf = suffixes.getOrNull(idx).orEmpty()
                    val variantName = when {
                        suf.isNotBlank() -> "$baseFullName $suf".trim()
                        files.size > 1 && idx > 0 -> "$baseFullName (#${idx + 1})"
                        else -> baseFullName
                    }
                    entries += AutoEqEntry(
                        name = variantName,
                        relPath = "${source.id}/$fileStem",
                        sourceRig = source.label,
                        sourceKind = EntrySourceKind.SQUIGLINK,
                        squigBaseUrl = source.baseUrl,
                        squigFileStem = fileStem,
                        formFactorOverride = source.defaultFormFactor,
                        rigTagOverride = source.rigTag,
                    )
                }
            }
        }
        return entries
    }

    /** Parse REW / Squiglink frequency response text file -> sorted (freqsHz, splDb). */
    fun parseSquigFrTxt(text: String): Pair<DoubleArray, DoubleArray> {
        val points = ArrayList<Pair<Double, Double>>(512)
        for (rawLine in text.lineSequence()) {
            val line = rawLine.trim()
            if (line.isEmpty() || line.startsWith("*") || line.startsWith("#") || line.startsWith(";")) continue
            val parts = line.split(Regex("[\\s,;]+"))
            if (parts.size < 2) continue
            val freq = parts[0].toDoubleOrNull() ?: continue
            val db = parts[1].toDoubleOrNull() ?: continue
            if (!freq.isFinite() || !db.isFinite()) continue
            if (freq in 10.0..25000.0) {
                points += freq to db
            }
        }
        require(points.size >= 10) { "Archivo FR Squiglink inválido o vacío." }
        points.sortBy { it.first }

        // Deduplicate identical frequencies keeping mean dB
        val outF = ArrayList<Double>(points.size)
        val outDb = ArrayList<Double>(points.size)
        var i = 0
        while (i < points.size) {
            val f = points[i].first
            var sum = points[i].second
            var cnt = 1
            while (i + 1 < points.size && abs(points[i + 1].first - f) < 1e-6) {
                i++
                sum += points[i].second
                cnt++
            }
            outF += f
            outDb += sum / cnt
            i++
        }
        return outF.toDoubleArray() to outDb.toDoubleArray()
    }

    fun parseGraphicEq(text: String): Pair<DoubleArray, DoubleArray> {
        var body = text.trim()
        if (body.lowercase().startsWith("graphiceq:")) {
            body = body.substringAfter(':')
        }
        val freqs = ArrayList<Double>(128)
        val gains = ArrayList<Double>(128)
        for (pair in body.split(';')) {
            val parts = pair.trim().split(Regex("\\s+"))
            if (parts.size >= 2) {
                val f = parts[0].toDoubleOrNull()
                val g = parts[1].toDoubleOrNull()
                if (f != null && g != null) {
                    freqs += f
                    gains += g
                }
            }
        }
        require(freqs.size >= 5) { "Formato GraphicEQ inválido o incompleto." }
        return freqs.toDoubleArray() to gains.toDoubleArray()
    }

    fun parseParametricEq(text: String): List<FilterBand> {
        val bands = mutableListOf<FilterBand>()
        for (line in text.lineSequence()) {
            val m = PARAM_RE.matchEntire(line.trim()) ?: continue
            val type = when (m.groupValues[1].uppercase()) {
                "LSC" -> FilterType.LOW_SHELF
                "HSC" -> FilterType.HIGH_SHELF
                else -> FilterType.PEAKING
            }
            val fc = m.groupValues[2].toDoubleOrNull() ?: continue
            val gain = m.groupValues[3].toDoubleOrNull() ?: continue
            val q = m.groupValues[4].toDoubleOrNull() ?: continue
            bands += FilterBand(type, fc, gain, q).clamped()
        }
        return bands
    }

    fun exportParametricEq(bands: List<FilterBand>): String {
        val sb = StringBuilder()
        sb.appendLine("Preamp: 0.0 dB")
        bands.forEachIndexed { idx, band ->
            val b = band.clamped()
            sb.appendLine(
                String.format(
                    java.util.Locale.US,
                    "Filter %d: ON %s Fc %d Hz Gain %.1f dB Q %.3f",
                    idx + 1,
                    b.type.apoCode,
                    b.frequency.roundToInt(),
                    b.gain,
                    b.q,
                )
            )
        }
        return sb.toString()
    }

    fun parseTargetCsv(text: String): Pair<DoubleArray, DoubleArray> {
        val freqs = ArrayList<Double>(256)
        val vals = ArrayList<Double>(256)
        val lines = text.lineSequence().drop(1)
        for (line in lines) {
            val parts = line.split(',')
            if (parts.size >= 2) {
                val f = parts[0].trim().toDoubleOrNull()
                val v = parts[1].trim().toDoubleOrNull()
                if (f != null && v != null) {
                    freqs += f
                    vals += v
                }
            }
        }
        require(freqs.isNotEmpty()) { "Target CSV vacío o inválido." }
        return freqs.toDoubleArray() to vals.toDoubleArray()
    }

    /** Find the best matching entry across all sources (AutoEq + Squiglink) for any headphone name. */
    fun findBestEntryAnyBrand(entries: List<AutoEqEntry>, hint: String): AutoEqEntry? {
        if (entries.isEmpty() || hint.isBlank()) return null
        val hintNorm = normalizeSearchText(hint)
        val tokens = hintNorm.split(' ').filter { it.length >= 2 }
        if (tokens.isEmpty()) return null
        var best: AutoEqEntry? = null
        var bestScore = 0
        for (e in entries) {
            val low = normalizeSearchText(e.name)
            val matched = tokens.count { it in low }
            if (matched == 0) continue
            var score = matched * 14
            if (matched == tokens.size) score += 18
            if (low == hintNorm) score += 60
            else if (low.startsWith(hintNorm)) score += 26
            else if (hintNorm in low) score += 16
            val rigLow = e.sourceRig.lowercase()
            when {
                "super* review" in rigLow -> score += 7
                "oratory1990" in rigLow -> score += 6
                "crinacle on 711" in rigLow -> score += 6
                "regancipher" in rigLow -> score += 5
                "aftersound" in rigLow -> score += 4
                "rtings" in rigLow -> score += 3
            }
            if (score > bestScore) {
                bestScore = score
                best = e
            }
        }
        return best
    }

    /** Normalize punctuation (e.g. Zero:3 -> zero 3) so user searches match reliably across rigs. */
    fun normalizeSearchText(raw: String): String {
        return raw.lowercase()
            .replace(':', ' ')
            .replace('-', ' ')
            .replace('_', ' ')
            .replace(Regex("\\s+"), " ")
            .trim()
    }

    fun centerMidband(freqs: DoubleArray, curveDb: DoubleArray): DoubleArray {
        var sum = 0.0
        var cnt = 0
        for (i in freqs.indices) {
            if (freqs[i] in 200.0..3000.0) {
                sum += curveDb[i]
                cnt++
            }
        }
        val offset = if (cnt > 0) sum / cnt else curveDb.average()
        return DoubleArray(curveDb.size) { curveDb[it] - offset }
    }

    fun normalize800Hz(freqs: DoubleArray, curveDb: DoubleArray): DoubleArray {
        var sum = 0.0
        var cnt = 0
        for (i in freqs.indices) {
            if (freqs[i] in 400.0..1200.0) {
                sum += curveDb[i]
                cnt++
            }
        }
        val offset = if (cnt > 0) sum / cnt else 0.0
        return DoubleArray(curveDb.size) { curveDb[it] - offset }
    }

    fun downsampleCurve(freqs: DoubleArray, db: DoubleArray, points: Int = 64): List<CurvePoint> {
        val grid = BiquadMath.logSpace(20.0, 20000.0, points)
        val interp = BiquadMath.logInterp(grid, freqs, db)
        return List(points) { i -> CurvePoint(grid[i], interp[i]) }
    }
}

class AutoEqRepository(context: Context) {

    private val cacheDir = File(context.cacheDir, "autoeq_v2").apply { mkdirs() }
    private val profilesDir = File(cacheDir, "profiles").apply { mkdirs() }
    private val targetsDir = File(cacheDir, "targets").apply { mkdirs() }
    private val json = Json { ignoreUnknownKeys = true }

    @Volatile
    private var memoryCatalog: CatalogLoadSummary? = null

    private val standardGrid = BiquadMath.logSpace(20.0, 20000.0, 128)

    suspend fun loadCatalog(forceRefresh: Boolean = false): CatalogLoadSummary = withContext(Dispatchers.IO) {
        if (!forceRefresh) {
            memoryCatalog?.let { return@withContext it }
        }
        val totalSources = 1 + AutoEqParser.SQUIG_SOURCES.size
        val jsonCache = File(cacheDir, "catalog_v2.json")
        if (!forceRefresh && jsonCache.exists()) {
            try {
                val parsed = json.decodeFromString<List<AutoEqEntry>>(jsonCache.readText())
                if (parsed.isNotEmpty()) {
                    val active = parsed.map { it.sourceKind to it.squigBaseUrl }.toSet().size
                    val summary = CatalogLoadSummary(parsed, active.coerceIn(1, totalSources), totalSources)
                    memoryCatalog = summary
                    return@withContext summary
                }
            } catch (_: Exception) {
            }
        }

        val combined = ArrayList<AutoEqEntry>(14000)
        var okSources = 0

        coroutineScope {
            val autoEqJob = async {
                try {
                    val md = fetchText(AutoEqParser.INDEX_URL, File(cacheDir, "autoeq_index.md"), forceRefresh, timeoutMs = 14000)
                    AutoEqParser.parseIndexMarkdown(md)
                } catch (_: Exception) {
                    emptyList()
                }
            }
            val squigJobs = AutoEqParser.SQUIG_SOURCES.map { src ->
                async {
                    try {
                        val txt = fetchText(
                            "${src.baseUrl}phone_book.json",
                            File(cacheDir, "squig_${src.id}.json"),
                            forceRefresh,
                            timeoutMs = 10000,
                        )
                        AutoEqParser.parseSquigPhoneBook(txt, src)
                    } catch (_: Exception) {
                        emptyList()
                    }
                }
            }

            // Put Squiglink newer community databases right alongside AutoEq so recent releases appear immediately
            val squigResults = squigJobs.awaitAll()
            for (list in squigResults) {
                if (list.isNotEmpty()) {
                    okSources++
                    combined.addAll(list)
                }
            }
            val aeList = autoEqJob.await()
            if (aeList.isNotEmpty()) {
                okSources++
                combined.addAll(aeList)
            }
        }

        if (combined.isEmpty()) {
            throw IllegalStateException("No se pudo descargar ninguna base de datos de curvas. Revisa tu conexión a internet.")
        }

        try {
            jsonCache.writeText(json.encodeToString(combined))
        } catch (_: Exception) {
        }

        val summary = CatalogLoadSummary(combined, okSources, totalSources)
        memoryCatalog = summary
        summary
    }

    suspend fun reconstructStockFr(entry: AutoEqEntry): List<CurvePoint> = withContext(Dispatchers.IO) {
        val (freqs, normFr) = fetchNormalizedMeasuredFr(entry)
        AutoEqParser.downsampleCurve(freqs, normFr, 64)
    }

    suspend fun computeTargetCurvePeq(
        source: AutoEqEntry,
        target: CuratedTarget,
        maxBands: Int,
        maxFreqFit: Double = 10000.0,
        maxGainDb: Double = 12.0,
        supportsShelves: Boolean = true,
    ): AutoEqResult = withContext(Dispatchers.Default) {
        val (fSrc, srcFr) = fetchNormalizedMeasuredFr(source)

        // Compute normalized target curve on fSrc
        val rawTarget = if (target.csvFileName == null) {
            defaultAutoEqBaseline(fSrc, source.formFactor)
        } else {
            val (fNew, newDb) = fetchTargetCsv(target.csvFileName)
            BiquadMath.logInterp(fSrc, fNew, newDb)
        }
        val targetFr = AutoEqParser.normalize800Hz(fSrc, rawTarget)

        // Exact desired EQ = targetFr - srcFr, centered in midband (200-3000 Hz)
        val desiredEq = AutoEqParser.centerMidband(
            fSrc,
            DoubleArray(fSrc.size) { targetFr[it] - srcFr[it] },
        )

        // Optional shortcut: for AutoEq default target on AutoEq entries, check pre-computed ParametricEQ
        var bands: List<FilterBand>? = null
        var rmse = 0.0
        if (source.sourceKind == EntrySourceKind.AUTOEQ_GITHUB &&
            target.csvFileName == null &&
            maxBands >= 5 &&
            supportsShelves &&
            maxGainDb >= 12.0
        ) {
            try {
                val preBands = fetchParametricEq(source).take(maxBands)
                if (preBands.size >= minOf(5, maxBands) && preBands.all { abs(it.gain) <= maxGainDb }) {
                    val resp = BiquadMath.combinedResponseDb(preBands, fSrc)
                    val respMid = AutoEqParser.centerMidband(fSrc, resp)
                    val diff = DoubleArray(fSrc.size) { desiredEq[it] - respMid[it] }
                    val w = BiquadMath.makeWeights(fSrc, maxFreqFit)
                    val preRmse = BiquadMath.weightedRmse(diff, w)
                    if (preRmse < 0.75) {
                        bands = padBands(preBands, maxBands)
                        rmse = preRmse
                    }
                }
            } catch (_: Exception) {
            }
        }

        if (bands == null) {
            val fitted = BiquadMath.fitPeqBands(fSrc, desiredEq, maxBands, maxFreqFit, maxGainDb, supportsShelves)
            bands = fitted.first
            rmse = fitted.second
        }

        AutoEqResult(
            bands = bands,
            rmseDb = rmse,
            warning = null,
            targetLabel = target.title,
            stockFrCurve = AutoEqParser.downsampleCurve(fSrc, srcFr, 64),
            targetFrCurve = AutoEqParser.downsampleCurve(fSrc, targetFr, 64),
        )
    }

    suspend fun computeMatchHeadphonePeq(
        source: AutoEqEntry,
        targetPhone: AutoEqEntry,
        maxBands: Int,
        maxFreqFit: Double = 10000.0,
        maxGainDb: Double = 12.0,
        supportsShelves: Boolean = true,
    ): AutoEqResult = withContext(Dispatchers.Default) {
        val (fSrc, srcFr) = fetchNormalizedMeasuredFr(source)
        val (fTgt, tgtFrRaw) = fetchNormalizedMeasuredFr(targetPhone)
        val tgtFr = BiquadMath.logInterp(fSrc, fTgt, tgtFrRaw)

        val deltaDb = AutoEqParser.centerMidband(
            fSrc,
            DoubleArray(fSrc.size) { tgtFr[it] - srcFr[it] },
        )
        val (bands, rmse) = BiquadMath.fitPeqBands(fSrc, deltaDb, maxBands, maxFreqFit, maxGainDb, supportsShelves)

        val warning = when {
            source.formFactor != targetPhone.formFactor ->
                "Aviso: comparando ${source.formFactor} vs ${targetPhone.formFactor}; el balance tonal relativo se mantiene, pero el oído percibe distinto over-ear vs IEM."
            source.rigTag != targetPhone.rigTag ->
                "Aviso: distinto equipo de medición (${source.rigTag} vs ${targetPhone.rigTag}); pueden variar los agudos >6 kHz."
            else -> null
        }

        AutoEqResult(
            bands = bands,
            rmseDb = rmse,
            warning = warning,
            targetLabel = targetPhone.name,
            stockFrCurve = AutoEqParser.downsampleCurve(fSrc, srcFr, 64),
            targetFrCurve = AutoEqParser.downsampleCurve(fSrc, tgtFr, 64),
        )
    }

    /**
     * Unified measured frequency response loader for both AutoEq GitHub and Squiglink sources.
     * Always returns (standardGrid, normalizedFrDb @ 400..1200 Hz = 0 dB).
     */
    private suspend fun fetchNormalizedMeasuredFr(entry: AutoEqEntry): Pair<DoubleArray, DoubleArray> = withContext(Dispatchers.IO) {
        when (entry.sourceKind) {
            EntrySourceKind.AUTOEQ_GITHUB -> {
                val (freqs, eqDb) = fetchGraphicEq(entry)
                val ref = defaultAutoEqBaseline(freqs, entry.formFactor)
                val fr = DoubleArray(freqs.size) { ref[it] - eqDb[it] }
                val interp = BiquadMath.logInterp(standardGrid, freqs, fr)
                standardGrid to AutoEqParser.normalize800Hz(standardGrid, interp)
            }

            EntrySourceKind.SQUIGLINK -> {
                val base = entry.squigBaseUrl
                val stem = entry.squigFileStem
                val digest = sha1Prefix("${entry.relPath}|$base", 16)

                val (leftCurve, rightCurve) = coroutineScope {
                    val leftJob = async {
                        tryFetchSquigChannel(base, "$stem L.txt", File(profilesDir, "${digest}_L.txt"))
                    }
                    val rightJob = async {
                        tryFetchSquigChannel(base, "$stem R.txt", File(profilesDir, "${digest}_R.txt"))
                    }
                    leftJob.await() to rightJob.await()
                }

                val merged: DoubleArray = when {
                    leftCurve != null && rightCurve != null -> {
                        val lI = BiquadMath.logInterp(standardGrid, leftCurve.first, leftCurve.second)
                        val rI = BiquadMath.logInterp(standardGrid, rightCurve.first, rightCurve.second)
                        DoubleArray(standardGrid.size) { (lI[it] + rI[it]) * 0.5 }
                    }
                    leftCurve != null -> BiquadMath.logInterp(standardGrid, leftCurve.first, leftCurve.second)
                    rightCurve != null -> BiquadMath.logInterp(standardGrid, rightCurve.first, rightCurve.second)
                    else -> {
                        val mono = tryFetchSquigChannel(base, "$stem.txt", File(profilesDir, "${digest}_M.txt"))
                            ?: throw IllegalStateException("No se pudo descargar la curva FR de ${entry.name} (${entry.sourceRig}).")
                        BiquadMath.logInterp(standardGrid, mono.first, mono.second)
                    }
                }
                standardGrid to AutoEqParser.normalize800Hz(standardGrid, merged)
            }
        }
    }

    private fun tryFetchSquigChannel(baseUrl: String, fileName: String, cacheFile: File): Pair<DoubleArray, DoubleArray>? {
        return try {
            val url = baseUrl + AutoEqParser.encodeUrlPathSegment(fileName)
            val txt = fetchText(url, cacheFile, forceRefresh = false, timeoutMs = 10000)
            AutoEqParser.parseSquigFrTxt(txt)
        } catch (_: Exception) {
            null
        }
    }

    private suspend fun fetchGraphicEq(entry: AutoEqEntry): Pair<DoubleArray, DoubleArray> = withContext(Dispatchers.IO) {
        val digest = sha1Prefix(entry.relPath, 16)
        val text = fetchText(entry.graphicEqUrl, File(profilesDir, "${digest}_graphic.txt"), false)
        AutoEqParser.parseGraphicEq(text)
    }

    private suspend fun fetchParametricEq(entry: AutoEqEntry): List<FilterBand> = withContext(Dispatchers.IO) {
        val digest = sha1Prefix(entry.relPath, 16)
        val text = fetchText(entry.parametricEqUrl, File(profilesDir, "${digest}_peq.txt"), false)
        AutoEqParser.parseParametricEq(text)
    }

    private suspend fun fetchTargetCsv(fileName: String): Pair<DoubleArray, DoubleArray> = withContext(Dispatchers.IO) {
        val encoded = AutoEqParser.encodeUrlPathSegment(fileName)
        val url = "${AutoEqParser.TARGETS_BASE_URL}/$encoded"
        val safe = sha1Prefix(fileName, 12) + ".csv"
        val text = fetchText(url, File(targetsDir, safe), false)
        AutoEqParser.parseTargetCsv(text)
    }

    private suspend fun defaultAutoEqBaseline(freqs: DoubleArray, formFactor: String): DoubleArray {
        if (formFactor == "over-ear") {
            val (fb, bDb) = fetchTargetCsv("Harman over-ear 2018.csv")
            return BiquadMath.logInterp(freqs, fb, bDb)
        }
        val (fb, bDb) = fetchTargetCsv("AutoEq in-ear.csv")
        val base = BiquadMath.logInterp(freqs, fb, bDb)
        val bassShelf = BiquadMath.bandResponseDb(FilterBand(FilterType.LOW_SHELF, 105.0, 8.0, 0.7), freqs)
        return DoubleArray(freqs.size) { base[it] + bassShelf[it] }
    }

    private fun padBands(bands: List<FilterBand>, targetCount: Int): List<FilterBand> {
        if (bands.size >= targetCount) return bands.take(targetCount)
        val out = bands.toMutableList()
        val pad = BiquadMath.flatBands(targetCount)
        while (out.size < targetCount) out += pad[out.size]
        return out
    }

    private fun fetchText(urlStr: String, cacheFile: File, forceRefresh: Boolean, timeoutMs: Int = 12000): String {
        if (!forceRefresh && cacheFile.exists() && cacheFile.length() > 0L) {
            return cacheFile.readText()
        }
        val conn = (URL(urlStr).openConnection() as HttpURLConnection).apply {
            connectTimeout = timeoutMs
            readTimeout = timeoutMs
            instanceFollowRedirects = true
            setRequestProperty("User-Agent", "AutoQ-Android/1.1")
        }
        return try {
            val code = conn.responseCode
            if (code !in 200..299) {
                throw IllegalStateException("HTTP $code descargando $urlStr")
            }
            val text = conn.inputStream.bufferedReader(Charsets.UTF_8).use { it.readText() }
            cacheFile.writeText(text)
            text
        } finally {
            conn.disconnect()
        }
    }

    private fun sha1Prefix(input: String, chars: Int): String {
        val bytes = MessageDigest.getInstance("SHA-1").digest(input.toByteArray(Charsets.UTF_8))
        return bytes.joinToString("") { "%02x".format(it) }.take(chars)
    }
}
