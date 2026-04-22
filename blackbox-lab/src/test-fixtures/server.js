const express = require('express');
const axios = require('axios');

const app = express();
const API_TIMEOUT = 5000;

// Line 6: Configure API client
const apiClient = axios.create({
    baseURL: 'http://external-api.service:8080',
    timeout: API_TIMEOUT,
    headers: { 'X-Service-ID': 'blackbox-lab' }
});

app.post('/api/webhook', async (req, res) => {
    try {
        // Line 15: Make external API call
        const response = await apiClient.post('/events', {
            event: req.body.event,
            timestamp: Date.now()
        });

        res.json({ success: true, data: response.data });
    } catch (error) {
        // Line 23: Handle timeout or connection refused
        if (error.code === 'ECONNREFUSED' || error.code === 'ETIMEDOUT') {
            console.error('External API unreachable:', error.message);
            res.status(503).json({ error: 'Service unavailable' });
        } else {
            throw error;
        }
    }
});

// Line 31: Request handler
app.get('/health', (req, res) => {
    res.json({ status: 'ok', uptime: process.uptime() });
});

module.exports = app;
