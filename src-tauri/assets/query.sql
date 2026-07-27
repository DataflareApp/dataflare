-- Welcome to the Dataflare demo database.
-- Chinook represents a digital music store with artists, albums, tracks, customers, and invoices.
-- Select either query below and run it to explore the data.

-- Demo 1: Browse tracks with their album, artist, and genre.
-- This query demonstrates how related tables can be combined with JOINs.
SELECT
    Track.Name AS Track,
    Album.Title AS Album,
    Artist.Name AS Artist,
    Genre.Name AS Genre,
    ROUND(Track.Milliseconds / 60000.0, 2) AS Minutes
FROM Track
JOIN Album ON Album.AlbumId = Track.AlbumId
JOIN Artist ON Artist.ArtistId = Album.ArtistId
LEFT JOIN Genre ON Genre.GenreId = Track.GenreId
ORDER BY Artist.Name, Album.Title, Track.TrackId
LIMIT 25;

-- Demo 2: Find the artists that generated the most sales.
-- This query demonstrates aggregation with COUNT, SUM, GROUP BY, and ORDER BY.
SELECT
    Artist.Name AS Artist,
    COUNT(DISTINCT Invoice.InvoiceId) AS Orders,
    SUM(InvoiceLine.Quantity) AS TracksSold,
    ROUND(SUM(InvoiceLine.UnitPrice * InvoiceLine.Quantity), 2) AS Revenue
FROM Artist
JOIN Album ON Album.ArtistId = Artist.ArtistId
JOIN Track ON Track.AlbumId = Album.AlbumId
JOIN InvoiceLine ON InvoiceLine.TrackId = Track.TrackId
JOIN Invoice ON Invoice.InvoiceId = InvoiceLine.InvoiceId
GROUP BY Artist.ArtistId, Artist.Name
ORDER BY Revenue DESC, Artist.Name
LIMIT 10;
