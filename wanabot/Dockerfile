FROM python:3.6.5-slim


COPY requirements.txt /opt
RUN apt-get update \
    && pip install --no-cache-dir -r /opt/requirements.txt \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

COPY . /opt
WORKDIR /opt
CMD python bot.py