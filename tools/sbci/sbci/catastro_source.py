"""STUB — Spanish Catastro (Sede Electrónica del Catastro) integration.

Not implemented. Documenting why rather than faking it:

The Catastro's open web service (OVCCoordenadas / OVCCallejero, under
`https://ovc.catastro.meh.es/ovcservweb/OVCSWLocalizacionRC/OVCCoordenadas.asmx`)
is a SOAP/XML service keyed to individual parcel references (referencia
catastral) or point coordinates, one parcel at a time — there's no bulk
"give me every parcel in this bbox with FAR" endpoint in the free tier.
Building a real per-cell FAR layer this way means either:

  1. Enumerating parcel references for the study area first (there isn't a
     public bulk list — this typically comes from INSPIRE cadastral ATOM
     feeds per municipality, a separate integration), then querying each
     one individually, or
  2. Using Catastro's INSPIRE WFS/WMS endpoints instead
     (`https://www.catastro.hacienda.gob.es/INSPIRE/...`), which do support
     bbox queries and are the more promising path — but weren't reachable
     to prototype against from this sandbox and haven't been tested here.

Either way this needs someone to actually test against the live service
before the pipeline can claim real per-parcel FAR data for Barcelona. Until
then, SCCI for Barcelona falls back to OSM building footprints only (same
signal as Norfolk), which is coarser than the brief's target
(footprint-only, no verified height/FAR) but not fabricated.
"""
from __future__ import annotations


def fetch_parcel_far(bbox_wgs84: tuple[float, float, float, float]):
    raise NotImplementedError(
        "Catastro integration not implemented — see module docstring for "
        "why and what the real integration path looks like."
    )
